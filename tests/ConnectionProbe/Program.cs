using VpnHood.Core.Client;
using VpnHood.Core.Client.Abstractions;
using VpnHood.Core.Common.Tokens;
using VpnHood.Net.Toolkit.Sockets;
using VpnHood.Net.VpnAdapters.Abstractions;

if (args.Length != 1)
	throw new ArgumentException("ConnectionProbe <private-access-key-file>");
var accessKey = File.ReadAllText(args[0]).Trim();
var options = new ClientOptions {
	AdapterName = "VpnHoodConnectionProbe",
	AccessKey = accessKey,
	ClientId = Guid.NewGuid().ToString(),
	AllowAnonymousTracker = false,
	AllowEndPointTracker = false,
	UseTcpProxy = true,
	EndPointStrategy = EndPointStrategy.IpOnly
};
using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(30));
await using var client = new VpnHoodClient(new NullVpnAdapter(autoDisposePackets: true, blocking: false),
	new SystemSocketFactory(), null, null, null, options);
await client.Connect(timeout.Token);
if (client.State != ClientState.Connected)
	throw new InvalidOperationException("İstemci bağlantısı kurulamadı.");
Console.WriteLine("PASS: gerçek VpnHood istemcisi Rust yöneticinin anahtarıyla uzak VpnHood sunucusuna bağlandı.");
Console.WriteLine("Sınır: NullVpnAdapter kullanıldığı için bu test VPN üzerinden kullanıcı trafiği taşımayı sınamaz.");
