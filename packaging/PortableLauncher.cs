// Compile as an x64 Windows GUI executable with the ZIP embedded as "payload.zip".
using System;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Reflection;
using System.Windows.Forms;

internal static class PortableLauncher
{
    [STAThread]
    private static int Main()
    {
        try
        {
            var assembly = Assembly.GetExecutingAssembly();
            var folder = Path.GetDirectoryName(assembly.Location);
            var portable = Path.Combine(folder, "Tizen Ornn Portable");
            Directory.CreateDirectory(portable);
            var runtime = Path.Combine(portable, "runtime-" + assembly.ManifestModule.ModuleVersionId);
            if (!Directory.Exists(runtime))
            {
                var staging = Directory.CreateDirectory(Path.Combine(portable, Guid.NewGuid().ToString()));
                try
                {
                    using (var stream = assembly.GetManifestResourceStream("payload.zip"))
                    using (var zip = new ZipArchive(stream))
                        zip.ExtractToDirectory(staging.FullName);
                    try { Directory.Move(staging.FullName, runtime); }
                    catch (IOException) { if (!Directory.Exists(runtime)) throw; }
                }
                finally
                {
                    if (Directory.Exists(staging.FullName)) Directory.Delete(staging.FullName, true);
                }
            }
            var gui = new ProcessStartInfo(Path.Combine(runtime, "tizen-ornn-gui.exe"));
            gui.WorkingDirectory = runtime;
            gui.UseShellExecute = false;
            gui.EnvironmentVariables["TIZEN_ORNN_PORTABLE_DIR"] = portable;
            gui.EnvironmentVariables["TIZEN_ORNN_QEMU_DIR"] = Path.Combine(runtime, "qemu");
            Process.Start(gui).Dispose();
            return 0;
        }
        catch (Exception error)
        {
            MessageBox.Show(error.Message, "Tizen Ornn", MessageBoxButtons.OK, MessageBoxIcon.Error);
            return 1;
        }
    }
}
