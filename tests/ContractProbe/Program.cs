using System.Net;
using System.Runtime.Versioning;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text.Json;
using System.Text.Json.Nodes;
using VpnHood.Core.Common.Messaging;
using VpnHood.Core.Common.Tokens;
using VpnHood.Core.Server.Access;
using VpnHood.Core.Server.Access.Configurations;
using VpnHood.Core.Server.Access.Managers.HttpAccessManagers;
using VpnHood.Core.Server.Access.Messaging;
using VpnHood.Net.Toolkit.Utils;

[assembly: SupportedOSPlatform("linux")]

if (args.Length < 2)
	throw new ArgumentException("prepare <work-directory> | probe <work-directory> [base-url]");
var work = Path.GetFullPath(args[1]);
Directory.CreateDirectory(work);
if (args[0] == "prepare") {
	using var cert = CertificateUtil.CreateSelfSigned("CN=vpnhood-test.invalid", DateTimeOffset.UtcNow.AddDays(30));
	var secret = RandomNumberGenerator.GetBytes(16);
	var config = new ServerConfig {
		ServerSecret = secret,
		TcpEndPoints = [new IPEndPoint(IPAddress.Any, 2443)],
		UdpEndPoints = [],
		QuicEndPoints = [],
		Certificates = [new CertificateData { CommonName = "vpnhood-test.invalid", RawData = cert.Export(X509ContentType.Pfx) }],
		UpdateStatusInterval = TimeSpan.FromSeconds(5),
		LogAnonymizer = true,
		TcpCongestionControl = null,
		SessionOptions = new SessionOptions { SyncInterval = TimeSpan.FromSeconds(5), SyncCacheSize = 1024 }
	};
	var options = new JsonSerializerOptions { PropertyNamingPolicy = JsonNamingPolicy.CamelCase };
	var node = JsonSerializer.SerializeToNode(config, options)!.AsObject();
	node["publicServerToken"] = JsonSerializer.SerializeToNode(new ServerToken {
		CreatedTime = DateTime.UtcNow,
		HostName = "vpnhood-test.invalid", HostPort = 2443, IsValidHostName = false,
		Secret = secret, CertificateHash = cert.GetCertHash(),
		HostEndPoints = [new IPEndPoint(IPAddress.Parse("10.0.3.220"), 2443)]
	});
	PrivateWrite(Path.Combine(work, "server-config.json"), node.ToJsonString());
	Console.WriteLine("PASS: .NET ServerConfig ve sertifika sınama dosyası hazırlandı.");
	return;
}
if (args[0] != "probe")
	throw new ArgumentException("Bilinmeyen komut.");
var baseUrl = new Uri(args.Length > 2 ? args[2] : "http://127.0.0.1:3080");
var credentialA = File.ReadAllText(Path.Combine(work, "server-a.credential")).Trim();
var credentialB = File.ReadAllText(Path.Combine(work, "server-b.credential")).Trim();
var tokenA = Token.FromAccessKey(File.ReadAllText(Path.Combine(work, "token-a.vh")));
var tokenB = Token.FromAccessKey(File.ReadAllText(Path.Combine(work, "token-b.vh")));
using var managerA = new HttpAccessManager(new HttpAccessManagerOptions(baseUrl, "Bearer " + credentialA));
using var managerB = new HttpAccessManager(new HttpAccessManagerOptions(baseUrl, "Bearer " + credentialB));
using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(30));
var ct = timeout.Token;
var info = new ServerInfo {
	Version = new Version("9.0.856"), EnvironmentVersion = Environment.Version,
	PrivateIpAddresses = [IPAddress.Loopback], PublicIpAddresses = [],
	Status = new ServerStatus(), MachineName = "contract-probe"
};
var configA = await managerA.Server_Configure(info, ct);
Check(configA.ServerSecret?.Length == 16 && configA.Certificates.Length == 1 && configA.UpdateStatusInterval == TimeSpan.FromSeconds(5), "ServerConfig gerçek .NET tipleriyle çözümlendi");
using (var http = new HttpClient()) {
	var unauthorized = await http.PostAsync(new Uri(baseUrl, "/api/agent/configure"), new StringContent("{}", System.Text.Encoding.UTF8, "application/json"), ct);
	Check(unauthorized.StatusCode == HttpStatusCode.Unauthorized, "kimliksiz sunucu reddedildi");
	http.DefaultRequestHeaders.Authorization = new("Bearer", credentialA[..^1] + (credentialA[^1] == 'A' ? "B" : "A"));
	var forged = await http.PostAsync(new Uri(baseUrl, "/api/agent/configure"), new StringContent("{}", System.Text.Encoding.UTF8, "application/json"), ct);
	Check(forged.StatusCode == HttpStatusCode.Unauthorized, "sahte sunucu kimliği reddedildi");
}
var clientA = "00112233-4455-6677-8899-aabbccddeeff";
var requestA = Request(tokenA, clientA);
var invalid = Request(tokenA, clientA);
invalid.EncryptedClientId[0] ^= 1;
Check((await managerA.Session_Create(invalid, ct)).ErrorCode == SessionErrorCode.AccessError, "hatalı istemci şifreli kanıtı reddedildi");
Check((await managerA.Session_Create(Request(tokenB, clientA), ct)).ErrorCode == SessionErrorCode.AccessError, "diğer kiracının anahtarı reddedildi");
if (File.Exists(Path.Combine(work, "token-expired.vh"))) {
	var expiredToken = Token.FromAccessKey(File.ReadAllText(Path.Combine(work, "token-expired.vh")));
	Check((await managerB.Session_Create(Request(expiredToken, clientA), ct)).ErrorCode == SessionErrorCode.AccessExpired, "süresi dolmuş anahtar reddedildi");
}
if (File.Exists(Path.Combine(work, "token-revoked.vh"))) {
	var revokedToken = Token.FromAccessKey(File.ReadAllText(Path.Combine(work, "token-revoked.vh")));
	Check((await managerB.Session_Create(Request(revokedToken, clientA), ct)).ErrorCode == SessionErrorCode.AccessLocked, "iptal edilmiş anahtar reddedildi");
}
var sessionA = await managerA.Session_Create(requestA, ct);
Check(sessionA.ErrorCode == SessionErrorCode.Ok && sessionA.SessionId > 0 && sessionA.SessionKey.Length == 16, "geçerli anahtarla oturum açıldı");
Check((await managerA.Session_Create(Request(tokenA, "11112233-4455-6677-8899-aabbccddeeff"), ct)).ErrorCode == SessionErrorCode.AccessLocked, "eşzamanlı cihaz kotası uygulandı");
var sessionB = await managerB.Session_Create(Request(tokenB, clientA), ct);
Check(sessionB.ErrorCode == SessionErrorCode.Ok, "ikinci kiracı bağımsız oturum açtı");
Check((await managerB.Session_Get(sessionA.SessionId, IPEndPoint.Parse("10.0.3.220:2443"), null, ct)).ErrorCode == SessionErrorCode.SessionError, "kiracılar arası oturum okuma engellendi");
var billed = await managerA.Session_AddUsage(sessionA.SessionId, new Traffic(1024, 512), null, ct);
Check(billed.AccessUsage?.CycleTraffic.Total == 1536, "tekli trafik farkı işlendi");
var status = new ServerStatus { SessionUsages = [new SessionUsage { SessionId = sessionA.SessionId, Sent = 512, Received = 512 }] };
var command = await managerA.Server_UpdateStatus(status, ct);
Check(command.ConfigCode == configA.ConfigCode && command.SessionResponses[sessionA.SessionId].AccessUsage?.CycleTraffic.Total == 2560, "durum bildirimi ve toplu kullanım işlendi");
var isolated = await managerB.Session_AddUsages([new SessionUsage { SessionId = sessionA.SessionId, Sent = 100, Received = 100 }], ct);
Check(isolated[sessionA.SessionId].ErrorCode == SessionErrorCode.SessionError, "diğer kiracının kullanım sayacı değiştirilemedi");
Check((await managerB.Session_Get(sessionB.SessionId, IPEndPoint.Parse("10.0.3.220:2443"), null, ct)).AccessUsage?.CycleTraffic.Total == 0, "ikinci kiracının sayacı bağımsız kaldı");
using (var http = new HttpClient()) {
	http.DefaultRequestHeaders.Authorization = new("Bearer", credentialA);
	var negative = await http.PostAsync(new Uri(baseUrl, $"/api/agent/sessions/{sessionA.SessionId}/usage?closeSession=false"), new StringContent("{\"sent\":-1,\"received\":0}", System.Text.Encoding.UTF8, "application/json"), ct);
	Check(negative.StatusCode == HttpStatusCode.BadRequest, "negatif kullanım reddedildi");
}
Check((await managerA.Session_Get(ulong.MaxValue, IPEndPoint.Parse("10.0.3.220:2443"), null, ct)).ErrorCode == SessionErrorCode.SessionError, "u64 sınırındaki bilinmeyen oturum güvenle reddedildi");
Check((await managerA.Session_GetAll(ct)).Any(s => s.SessionId == sessionA.SessionId), "oturum kurtarma listesi uyumlu");
var ended = await managerA.Session_Close(sessionA.SessionId, new Traffic(), ct);
Check(ended.ErrorCode == SessionErrorCode.SessionClosed, "oturum kapatma işlendi");
var reopened = await managerA.Session_Create(Request(tokenA, clientA), ct);
Check(reopened.ErrorCode == SessionErrorCode.Ok && reopened.SessionId != sessionA.SessionId, "kapatılan oturum cihaz kotasını serbest bıraktı");
var overflow = await managerA.Session_AddUsages([new SessionUsage { SessionId = reopened.SessionId, Sent = 1024, Received = 1024 }], ct);
Check(overflow[reopened.SessionId].ErrorCode == SessionErrorCode.AccessTrafficOverflow, "trafik kotası uygulandı");
Check((await managerA.Session_Create(Request(tokenA, clientA), ct)).ErrorCode == SessionErrorCode.AccessTrafficOverflow, "kota dolunca yeni oturum engellendi");
await managerB.Session_Close(sessionB.SessionId, new Traffic(), ct);
var attempts = await Task.WhenAll(Enumerable.Range(0, 16).Select(_ => managerB.Session_Create(Request(tokenB, Guid.NewGuid().ToString()), ct)));
Check(attempts.Count(s => s.ErrorCode == SessionErrorCode.Ok) == 1 && attempts.Count(s => s.ErrorCode == SessionErrorCode.AccessLocked) == 15, "16 eşzamanlı istekte cihaz kotası atomik korundu");
await managerB.Session_Close(attempts.Single(s => s.ErrorCode == SessionErrorCode.Ok).SessionId, new Traffic(), ct);
Console.WriteLine("PASS: gerçek VpnHood HttpAccessManager ile bütün sözleşme sınamaları tamamlandı.");

static SessionRequestEx Request(Token token, string clientId) => new() {
	TokenId = token.TokenId,
	ClientInfo = new ClientInfo { ClientId = clientId, ClientVersion = "9.0.856", UserAgent = "ContractProbe", MinProtocolVersion = 10, MaxProtocolVersion = 10 },
	EncryptedClientId = VhUtils.EncryptClientId(clientId, token.Secret),
	HostEndPoint = IPEndPoint.Parse("10.0.3.220:2443"), ClientIp = IPAddress.Loopback,
	RemoteEndPoint = IPEndPoint.Parse("127.0.0.1:50000"), ExtraData = null, ProtocolVersion = 10
};
static void Check(bool condition, string name) {
	if (!condition) throw new InvalidOperationException("FAIL: " + name);
	Console.WriteLine("PASS: " + name);
}
static void PrivateWrite(string path, string content) {
	using var stream = new FileStream(path, new FileStreamOptions { Access = FileAccess.Write, Mode = FileMode.CreateNew, UnixCreateMode = UnixFileMode.UserRead | UnixFileMode.UserWrite });
	using var writer = new StreamWriter(stream);
	writer.Write(content);
}
