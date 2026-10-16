using System.Diagnostics;
using System.Net;
using System.Net.Quic;
using System.Net.Security;
using System.Net.Sockets;
using System.Runtime.Versioning;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text;
using VpnHood.Core.Client;
using VpnHood.Core.Client.Abstractions;
using VpnHood.Core.Common.Messaging;
using VpnHood.Core.Common.Tokens;
using VpnHood.Core.Tunneling.Messaging;
using VpnHood.Core.Tunneling.Utils;
using VpnHood.Net.Toolkit.Sockets;
using VpnHood.Net.Toolkit.Streams;
using VpnHood.Net.VpnAdapters.Abstractions;

[assembly: SupportedOSPlatform("linux")]

if (args.Length is not (1 or 5))
	throw new ArgumentException("ConnectionProbe <private-access-key-file> [tcp|quic|blocked destination-ip port byte-count]");
var accessKey = File.ReadAllText(args[0]).Trim();
var token = Token.FromAccessKey(accessKey);
var options = new ClientOptions {
	AdapterName = "VpnHoodConnectionProbe",
	AccessKey = accessKey,
	ClientId = Guid.NewGuid().ToString(),
	AllowAnonymousTracker = false,
	AllowEndPointTracker = false,
	UseTcpProxy = true,
	EndPointStrategy = EndPointStrategy.IpOnly
};
using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(60));
await using var client = new VpnHoodClient(new NullVpnAdapter(autoDisposePackets: true, blocking: false),
	new SystemSocketFactory(), null, null, null, options);
await client.Connect(timeout.Token);
if (client.State != ClientState.Connected)
	throw new InvalidOperationException("İstemci bağlantısı kurulamadı.");
Console.WriteLine("PASS: gerçek VpnHood istemcisi Rust yöneticinin anahtarıyla uzak VpnHood sunucusuna bağlandı.");
if (args.Length == 1) {
	Console.WriteLine("Sınır: Bu kipte NullVpnAdapter kullanıcı trafiği taşımaz.");
	return;
}
var config = client.RequiredSession.Config;
RemoteCertificateValidationCallback pin = (_, cert, _, _) => cert != null &&
	token.ServerToken.CertificateHash is { Length: > 0 } hash &&
	CryptographicOperations.FixedTimeEquals(cert.GetCertHash(), hash);
var mode = args[1];
QuicConnection? quic = null;
TcpClient? tcp = null;
Stream stream;
try {
	if (mode == "quic") {
		if (!QuicConnection.IsSupported || config.HostQuicEndPoint is null)
			throw new InvalidOperationException("QUIC veya sunucu duyurusu eksik.");
		quic = await QuicConnection.ConnectAsync(new QuicClientConnectionOptions {
			RemoteEndPoint = config.HostQuicEndPoint,
			DefaultStreamErrorCode = 0, DefaultCloseErrorCode = 0,
			ClientAuthenticationOptions = new SslClientAuthenticationOptions {
				TargetHost = token.ServerToken.HostName,
				ApplicationProtocols = [SslApplicationProtocol.Http3],
				RemoteCertificateValidationCallback = pin
			}
		}, timeout.Token);
		stream = await quic.OpenOutboundStreamAsync(QuicStreamType.Bidirectional, timeout.Token);
	} else {
		tcp = new TcpClient();
		await tcp.ConnectAsync(config.HostTcpEndPoint!, timeout.Token);
		var ssl = new SslStream(tcp.GetStream(), false, pin);
		await ssl.AuthenticateAsClientAsync(new SslClientAuthenticationOptions {
			TargetHost = token.ServerToken.HostName
		}, timeout.Token);
		stream = ssl;
	}
	await using (stream) {
		var request = new StreamProxyChannelRequest {
			RequestId = Guid.NewGuid().ToString(), SessionId = config.SessionId,
			SessionKey = config.SessionKey,
			DestinationEndPoint = new IPEndPoint(IPAddress.Parse(args[2]), int.Parse(args[3]))
		};
		var body = StreamUtils.ObjectToJsonBuffer(request);
		var headers = Encoding.ASCII.GetBytes($"POST / HTTP/1.1\r\nHost: {token.ServerToken.HostName}\r\nX-ProtocolVersion: 13\r\nX-BinaryStream: None\r\nContent-Length: {body.Length + 2}\r\n\r\n");
		await stream.WriteAsync(headers, timeout.Token);
		await stream.WriteAsync(new byte[] { 1, request.RequestCode }, timeout.Token);
		await stream.WriteAsync(body, timeout.Token);
		await stream.FlushAsync(timeout.Token);
		var httpResponse=await HttpUtils.ReadResponse(stream, timeout.Token);
		if(mode=="blocked" && httpResponse.StatusCode==HttpStatusCode.Unauthorized) {
			Console.WriteLine("PASS: hazır sunucu engellenen proxy hedefini HTTP 401 ile reddetti.");
			return;
		}
		if(httpResponse.StatusCode!=HttpStatusCode.OK) throw new InvalidOperationException($"Tünel HTTP yanıtı: {httpResponse.StatusCode}");
		var response = await StreamUtils.ReadObjectAsync<SessionResponse>(stream, timeout.Token);
		if (mode == "blocked") {
			if (response.ErrorCode == SessionErrorCode.Ok || response.ErrorMessage?.Contains("NetFilter") != true)
				throw new InvalidOperationException("Beklenen ağ filtresi engeli alınmadı.");
			Console.WriteLine("PASS: gerçek sunucu NetFilter hedefini engelledi.");
			return;
		}
		if (response.ErrorCode != SessionErrorCode.Ok)
			throw new InvalidOperationException($"Proxy kanalı reddedildi: {response.ErrorCode}, {response.ErrorMessage}");
		var count = int.Parse(args[4]);
		var watch = Stopwatch.StartNew();
		await stream.WriteAsync(new byte[] { 1 }, timeout.Token);
		var buffer = new byte[16384];
		var received = 0;
		while (received < count) {
			var read = await stream.ReadAsync(buffer.AsMemory(0, Math.Min(buffer.Length, count-received)), timeout.Token);
			if (read == 0) throw new EndOfStreamException("Veri aktarımı tamamlanmadı.");
			for (var i=0; i<read; i++)
				if (buffer[i] != (byte)0x5a) throw new InvalidDataException("Aktarılan veri eşleşmiyor.");
			received += read;
			if(mode=="tcp" && received-read < 10*1024*1024 && received>=10*1024*1024) watch.Restart();
		}
		watch.Stop();
		if (mode == "tcp" && config.MaxSpeedMbps?.Received == 1 && (watch.Elapsed.TotalSeconds < 8 || watch.Elapsed.TotalSeconds > 40))
			throw new InvalidOperationException($"1 Mbps sınaması: {received} bayt, {watch.Elapsed.TotalSeconds:F2} sn; beklenen 8–40 sn.");
		Console.WriteLine($"PASS: {mode.ToUpperInvariant()} proxy kanalında {received} bayt doğrulandı; süre {watch.Elapsed.TotalSeconds:F2} sn.");
	}
} finally {
	tcp?.Dispose();
	if (quic != null) await quic.DisposeAsync();
}
