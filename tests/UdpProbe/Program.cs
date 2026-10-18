using System.Buffers.Binary;
using System.Net;
using VpnHood.Core.Client;
using VpnHood.Core.Client.Abstractions;
using VpnHood.Core.Common.Messaging;
using VpnHood.Net.Packets;
using VpnHood.Net.Packets.Extensions;
using VpnHood.Net.Toolkit.Net;
using VpnHood.Net.Quic.MsQuic;
using VpnHood.Net.VpnAdapters.Abstractions;

if(args.Length!=2) throw new ArgumentException("UdpProbe <private-key-file> Tcp|Udp|Quic");
var requested=Enum.Parse<ChannelProtocol>(args[1],true);
var adapter=new ProbeAdapter();
var options=new ClientOptions {
	AdapterName="HoodminUdpProbe",AccessKey=File.ReadAllText(args[0]).Trim(),
	ClientId=Guid.NewGuid().ToString(),AllowAnonymousTracker=false,AllowEndPointTracker=false,
	ChannelProtocol=requested,UseTcpProxy=false
};
using var timeout=new CancellationTokenSource(TimeSpan.FromSeconds(45));
await using var client=new VpnHoodClient(adapter,new MsQuicSocketFactory(),null,null,null,options);
await client.Connect(timeout.Token);
if(client.State!=ClientState.Connected || client.RequiredSession.Status.ChannelProtocol!=requested)
	throw new InvalidOperationException("Requested channel was not negotiated.");
byte[] query=Convert.FromHexString("48FA01000001000000000000076578616D706C6503636F6D0000010001");
Console.WriteLine($"INFO: Kanal {client.RequiredSession.Status.ChannelProtocol}; adapter {client.RequiredSession.Status.IsAdapterStarted}; UDP destek {client.RequiredSession.Info.IsUdpChannelSupported}; UDP uç {client.RequiredSession.Config.HostUdpEndPoint}; paket kanalı {client.RequiredSession.Status.ActivePacketChannelCount}");
var packet=PacketBuilder.BuildUdp(client.RequiredSession.Config.AdapterOptions.VirtualIpNetworkV4!.Prefix,IPAddress.Parse("1.1.1.1"),53123,53,query);
packet.UpdateAllChecksums();
adapter.Emit(packet);
byte[] response;
try { response=await adapter.Response.Task.WaitAsync(timeout.Token); }
catch { Console.WriteLine($"INFO: TX/RX {client.RequiredSession.Status.SessionTraffic}; kanallar {client.RequiredSession.Status.ActivePacketChannelCount}; adapter yanıtları {adapter.Packets}; son kaynak {adapter.LastSource}");throw; }
if(response.Length<12 || BinaryPrimitives.ReadUInt16BigEndian(response)!=0x48fa || (response[2]&0x80)==0 || (response[3]&15)!=0 || BinaryPrimitives.ReadUInt16BigEndian(response.AsSpan(6,2))==0)
	throw new InvalidDataException("DNS reply was not a successful matching response.");
Console.WriteLine($"PASS: {requested} VPN kanalından gerçek UDP DNS yanıtı alındı ({response.Length} bayt); protokol geri düşmesi yok.");

sealed class ProbeAdapter():NullVpnAdapter(autoDisposePackets:true,blocking:false) {
	public int Packets;
	public string? LastSource;
	public TaskCompletionSource<byte[]> Response {get;}=new(TaskCreationOptions.RunContinuationsAsynchronously);
	public void Emit(IpPacket packet)=>OnPacketReceived(packet);
	protected override bool WritePacket(IpPacket packet) {
		Packets++;LastSource=packet.SourceAddress.ToString();
		if(packet.Protocol==IpProtocol.Udp && packet.SourceAddress.Equals(IPAddress.Parse("1.1.1.1"))) {
			var udp=packet.ExtractUdp();
			if(udp.SourcePort==53 && udp.DestinationPort==53123) Response.TrySetResult(udp.Payload.ToArray());
		}
		return true;
	}
}
