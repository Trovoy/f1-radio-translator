using System;
using System.Diagnostics;
using System.ComponentModel;
using System.Drawing;
using System.IO;
using System.IO.Compression;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;

[assembly: AssemblyTitle("MultiViewer F1 Radio Translator")]
[assembly: AssemblyDescription("MultiViewer F1 无线电实时翻译")]
[assembly: AssemblyProduct("MultiViewer F1 Radio Translator")]
[assembly: AssemblyVersion("1.0.2.0")]
[assembly: AssemblyFileVersion("1.0.2.0")]

internal static class Launcher
{
    private static readonly string[] RequiredFiles = {
        "Start-MultiViewerTranslator.ps1", "RegionSelector.dll", "rapidocr_worker.py",
        "python-runtime\\python.exe", "python-runtime\\python312.dll",
        "python-runtime\\Lib\\encodings\\__init__.py", "python-deps\\rapidocr\\__init__.py"
    };

    [STAThread]
    private static void Main()
    {
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        try
        {
            string releaseId;
            using (Stream stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("Translator.PayloadId"))
            using (StreamReader reader = new StreamReader(stream, Encoding.UTF8))
                releaseId = reader.ReadToEnd().Trim();
            string appRoot = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                                          "MultiViewerRadioTranslator", "app", releaseId);
            EnsurePayload(appRoot, releaseId);
            StartApplication(appRoot);
        }
        catch (Exception error)
        {
            MessageBox.Show("无法启动翻译器：\n" + error.Message,
                "MultiViewer F1 翻译器", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
    }

    private static bool IsReady(string appRoot, string releaseId)
    {
        string marker = Path.Combine(appRoot, "payload.ready");
        if (!File.Exists(marker) || File.ReadAllText(marker, Encoding.UTF8).Trim() != releaseId) return false;
        foreach (string relative in RequiredFiles)
        {
            string file = Path.Combine(appRoot, relative);
            if (!File.Exists(file) || new FileInfo(file).Length == 0) return false;
        }
        return true;
    }

    private static void EnsurePayload(string appRoot, string releaseId)
    {
        if (IsReady(appRoot, releaseId)) return;
        using (Mutex extractionGate = new Mutex(false, "Local\\MultiViewerRadioTranslator_Extract_" + releaseId))
        using (StartupWindow startup = new StartupWindow())
        {
            startup.Show();
            startup.SetProgress(0, "首次启动：正在准备本地 OCR 和运行环境…");
            bool ownsGate = false;
            try
            {
                while (!ownsGate)
                {
                    try { ownsGate = extractionGate.WaitOne(100); }
                    catch (AbandonedMutexException) { ownsGate = true; }
                    Application.DoEvents();
                }
                if (IsReady(appRoot, releaseId)) return;
                Directory.CreateDirectory(appRoot);
                // The embedded archive is immutable; only extract inside this version's cache.
                string rootPrefix = Path.GetFullPath(appRoot).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
                using (Stream payload = Assembly.GetExecutingAssembly().GetManifestResourceStream("Translator.Payload"))
                using (ZipArchive archive = new ZipArchive(payload, ZipArchiveMode.Read))
                {
                    long total = 0;
                    foreach (ZipArchiveEntry entry in archive.Entries) total += entry.Length;
                    long completed = 0;
                    byte[] buffer = new byte[1024 * 1024];
                    foreach (ZipArchiveEntry entry in archive.Entries)
                    {
                        string target = Path.GetFullPath(Path.Combine(appRoot, entry.FullName.Replace('/', Path.DirectorySeparatorChar)));
                        if (!target.StartsWith(rootPrefix, StringComparison.OrdinalIgnoreCase))
                            throw new InvalidDataException("应用包包含无效文件路径。");
                        if (String.IsNullOrEmpty(entry.Name)) { Directory.CreateDirectory(target); continue; }
                        Directory.CreateDirectory(Path.GetDirectoryName(target));
                        using (Stream input = entry.Open())
                        using (FileStream output = new FileStream(target, FileMode.Create, FileAccess.Write, FileShare.None))
                        {
                            int count;
                            while ((count = input.Read(buffer, 0, buffer.Length)) > 0)
                            {
                                output.Write(buffer, 0, count);
                                completed += count;
                                startup.SetProgress((int)(completed * 100 / Math.Max(1, total)), "正在准备本地运行环境…");
                            }
                        }
                    }
                }
                foreach (string required in RequiredFiles)
                    if (!File.Exists(Path.Combine(appRoot, required)))
                        throw new InvalidDataException("应用包缺少运行文件：" + required);
                File.WriteAllText(Path.Combine(appRoot, "payload.ready"), releaseId, new UTF8Encoding(false));
                startup.SetProgress(100, "准备完成，正在打开翻译器…");
            }
            finally { if (ownsGate) extractionGate.ReleaseMutex(); }
        }
    }

    private static void StartApplication(string appRoot)
    {
        string windowsRoot = Environment.GetFolderPath(Environment.SpecialFolder.Windows);
        ProcessStartInfo info = new ProcessStartInfo();
        info.FileName = Path.Combine(windowsRoot, "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
        info.Arguments = "-NoLogo -NoProfile -STA -WindowStyle Hidden -ExecutionPolicy Bypass -File \"" +
                         Path.Combine(appRoot, "Start-MultiViewerTranslator.ps1") + "\"";
        info.WorkingDirectory = appRoot;
        info.UseShellExecute = false;
        info.CreateNoWindow = true;
        info.WindowStyle = ProcessWindowStyle.Hidden;
        info.EnvironmentVariables["PYTHONHOME"] = Path.Combine(appRoot, "python-runtime");
        info.EnvironmentVariables["PYTHONPATH"] = Path.Combine(appRoot, "python-deps");
        info.EnvironmentVariables["PYTHONNOUSERSITE"] = "1";
        info.EnvironmentVariables["PYTHONUTF8"] = "1";
        using (ApplicationProcessJob job = new ApplicationProcessJob())
        using (Process app = Process.Start(info))
        {
            if (app == null) throw new InvalidOperationException("无法创建应用进程。");
            try
            {
                job.Attach(app);
                app.WaitForExit();
            }
            catch
            {
                try { if (!app.HasExited) { app.Kill(); app.WaitForExit(2000); } } catch { }
                throw;
            }
            if (app.ExitCode != 0)
                throw new InvalidOperationException("界面进程意外退出（代码 " + app.ExitCode + "）。请重新运行。");
        }
    }
}

// Windows owns the complete app process tree. Closing the launcher or returning
// from the UI process releases this job and terminates any remaining workers.
internal sealed class ApplicationProcessJob : IDisposable
{
    private IntPtr handle;
    [StructLayout(LayoutKind.Sequential)]
    private struct BasicLimits
    {
        public long ProcessTime, JobTime;
        public uint Flags;
        public UIntPtr MinimumWorkingSet, MaximumWorkingSet;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass, SchedulingClass;
    }
    [StructLayout(LayoutKind.Sequential)]
    private struct IoCounters
    {
        public ulong ReadOperations, WriteOperations, OtherOperations;
        public ulong ReadBytes, WriteBytes, OtherBytes;
    }
    [StructLayout(LayoutKind.Sequential)]
    private struct ExtendedLimits
    {
        public BasicLimits Basic;
        public IoCounters Io;
        public UIntPtr ProcessMemory, JobMemory, PeakProcessMemory, PeakJobMemory;
    }
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    private static extern IntPtr CreateJobObject(IntPtr attributes, string name);
    [DllImport("kernel32.dll", SetLastError=true)]
    private static extern bool SetInformationJobObject(IntPtr job, int type, ref ExtendedLimits limits, uint size);
    [DllImport("kernel32.dll", SetLastError=true)]
    private static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);
    [DllImport("kernel32.dll", SetLastError=true)] private static extern bool CloseHandle(IntPtr handle);

    internal ApplicationProcessJob()
    {
        handle = CreateJobObject(IntPtr.Zero, null);
        if (handle == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error());
        ExtendedLimits limits = new ExtendedLimits();
        limits.Basic.Flags = 0x2000; // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if (!SetInformationJobObject(handle, 9, ref limits, (uint)Marshal.SizeOf(typeof(ExtendedLimits))))
        {
            int error = Marshal.GetLastWin32Error();
            Dispose();
            throw new Win32Exception(error);
        }
    }
    internal void Attach(Process process)
    {
        if (!AssignProcessToJobObject(handle, process.Handle))
            throw new Win32Exception(Marshal.GetLastWin32Error(), "无法建立应用进程退出保护。");
    }
    public void Dispose()
    {
        if (handle != IntPtr.Zero) { CloseHandle(handle); handle = IntPtr.Zero; }
    }
}

internal sealed class StartupWindow : Form
{
    private readonly Label status;
    private readonly ProgressBar progress;
    private int shownProgress = -1;

    internal StartupWindow()
    {
        Text = "MultiViewer F1 翻译器";
        ClientSize = new Size(420, 152);
        FormBorderStyle = FormBorderStyle.FixedDialog;
        StartPosition = FormStartPosition.CenterScreen;
        MaximizeBox = false;
        MinimizeBox = false;
        ControlBox = false;
        BackColor = Color.FromArgb(17, 23, 32);
        ForeColor = Color.FromArgb(237, 244, 252);
        Font = new Font("Microsoft YaHei UI", 10f);
        Icon = Icon.ExtractAssociatedIcon(Assembly.GetExecutingAssembly().Location);
        PictureBox logo = new PictureBox { Location = new Point(24, 18), Size = new Size(44, 44), SizeMode = PictureBoxSizeMode.Zoom };
        using (Stream stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("Translator.Logo"))
        using (Image image = Image.FromStream(stream)) logo.Image = new Bitmap(image);
        Label title = new Label { Text = "无线电实时翻译", AutoSize = true,
            Location = new Point(80, 23), Font = new Font("Microsoft YaHei UI", 15f, FontStyle.Bold),
            ForeColor = Color.FromArgb(86, 199, 242) };
        status = new Label { AutoSize = false, Location = new Point(24, 62), Size = new Size(372, 24) };
        progress = new ProgressBar { Location = new Point(24, 103), Size = new Size(372, 10),
            Style = ProgressBarStyle.Continuous };
        Controls.Add(logo); Controls.Add(title); Controls.Add(status); Controls.Add(progress);
    }

    internal void SetProgress(int percent, string text)
    {
        percent = Math.Max(0, Math.Min(100, percent));
        if (percent == shownProgress) return;
        shownProgress = percent;
        status.Text = text;
        progress.Value = percent;
        Application.DoEvents();
    }
}
