using GameReaderCommon;
using SimHub.Plugins;
using System;
using System.Diagnostics;
using System.Globalization;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading;
using System.Web.Script.Serialization;

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
        private string game = "", carId = "", carModel = "", carClass = "";
        private readonly JavaScriptSerializer serializer = new JavaScriptSerializer();
        private static string Clean(string value, int max) {
            if (string.IsNullOrEmpty(value)) return "";
            var text = new StringBuilder();
            foreach (char c in value) if (!char.IsControl(c) && text.Length < max) text.Append(c);
            return text.ToString();
        }

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
                game = running ? Clean(data.GameName, 32) : "";
                carId = running ? Clean(data.NewData.CarId, 120) : "";
                carModel = running ? Clean(data.NewData.CarModel, 120) : "";
                carClass = running ? Clean(data.NewData.CarClass, 120) : "";
                updated = Stopwatch.GetTimestamp();
            }
        }

        private void Send(object state)
        {
            lock (gate)
            {
                if (udp == null) return;
                bool fresh = (Stopwatch.GetTimestamp() - updated) / (double)Stopwatch.Frequency < 1;
                bool active = running && fresh;
                string json = serializer.Serialize(new {
                    version = 2, running = active, speed_kmh = active ? speed : 0,
                    game = active ? game : "", car_id = active ? carId : "",
                    car_model = active ? carModel : "", car_class = active ? carClass : ""
                });
                byte[] bytes = Encoding.UTF8.GetBytes(json);
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
