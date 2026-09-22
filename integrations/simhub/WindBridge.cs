using GameReaderCommon;
using SimHub.Plugins;
using System;
using System.Diagnostics;
using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading;

namespace RigCompanion.WindBridge
{
    [PluginDescription("Sends vehicle speed to Rig Companion on this PC. Rig Companion controls the wind simulator USB connection and airflow limits.")]
    [PluginAuthor("LINDESTAD")]
    [PluginName("Rig Companion Wind Bridge")]
    public sealed class WindBridge : IPlugin, IDataPlugin
    {
        public PluginManager PluginManager { get; set; }
        private readonly object gate = new object();
        private UdpClient udp;
        private Timer timer;
        private bool running;
        private double speed;
        private long updated;

        public void Init(PluginManager pluginManager)
        {
            lock (gate)
            {
                PluginManager = pluginManager;
                udp = new UdpClient(AddressFamily.InterNetwork);
                udp.Connect(IPAddress.Loopback, 29814);
                udp.Client.Blocking = false;
                timer = new Timer(Send, null, 0, 100);
            }
        }

        public void DataUpdate(PluginManager pluginManager, ref GameData data)
        {
            // No network or disk I/O on SimHub's game-data callback.
            lock (gate)
            {
                running = data != null && data.GameRunning && data.NewData != null;
                speed = running ? Convert.ToDouble(data.NewData.SpeedKmh, CultureInfo.InvariantCulture) : 0;
                if (double.IsNaN(speed) || double.IsInfinity(speed)) { speed = 0; running = false; }
                speed = Math.Max(0, Math.Min(1500, speed));
                updated = Stopwatch.GetTimestamp();
            }
        }

        private void Send(object state)
        {
            lock (gate)
            {
                if (udp == null) return;
                bool fresh = (Stopwatch.GetTimestamp() - updated) / (double)Stopwatch.Frequency < 1;
                string json = "{\"version\":1,\"running\":" + (running && fresh ? "true" : "false")
                    + ",\"speed_kmh\":" + (running && fresh ? speed : 0).ToString("0.###", CultureInfo.InvariantCulture) + "}";
                byte[] bytes = Encoding.ASCII.GetBytes(json);
                try { udp.Send(bytes, bytes.Length); }
                catch (SocketException) { /* Companion closed or temporarily busy; next heartbeat retries. */ }
                catch (ObjectDisposedException) { }
            }
        }

        public void End(PluginManager pluginManager)
        {
            lock (gate)
            {
                if (timer != null) { timer.Dispose(); timer = null; }
                if (udp != null) { udp.Dispose(); udp = null; }
                running = false;
            }
        }
    }
}
