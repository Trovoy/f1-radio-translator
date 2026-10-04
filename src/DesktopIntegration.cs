using System;
using System.IO;
using System.Runtime.InteropServices;

namespace F1RadioTranslator
{
    public static class DesktopIntegration
    {
        public const string ApplicationId = "Trovoy.F1RadioTranslator";
        private static readonly Guid PropertyFormat = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3");

        [StructLayout(LayoutKind.Sequential)]
        private struct PropertyKey
        {
            public Guid Format;
            public uint Id;
            public PropertyKey(uint id) { Format = PropertyFormat; Id = id; }
        }

        // Windows x64 PROPVARIANT: eight-byte header and sixteen-byte union.
        [StructLayout(LayoutKind.Explicit, Size = 24)]
        private struct PropVariant
        {
            [FieldOffset(0)] public ushort Type;
            [FieldOffset(8)] public IntPtr Text;
        }

        [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IPropertyStore
        {
            [PreserveSig] int GetCount(out uint count);
            [PreserveSig] int GetAt(uint index, out PropertyKey key);
            [PreserveSig] int GetValue(ref PropertyKey key, out PropVariant value);
            [PreserveSig] int SetValue(ref PropertyKey key, ref PropVariant value);
            [PreserveSig] int Commit();
        }

        [DllImport("shell32.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
        private static extern int SetCurrentProcessExplicitAppUserModelID(string id);
        [DllImport("shell32.dll", ExactSpelling = true)]
        private static extern int SHGetPropertyStoreForWindow(IntPtr window, ref Guid iid, out IPropertyStore store);
        [DllImport("ole32.dll", ExactSpelling = true)]
        private static extern int PropVariantClear(ref PropVariant value);

        public static void SetProcessIdentity()
        {
            Marshal.ThrowExceptionForHR(SetCurrentProcessExplicitAppUserModelID(ApplicationId));
        }

        private static IPropertyStore GetStore(IntPtr window)
        {
            if (window == IntPtr.Zero) throw new ArgumentException("A live window handle is required.");
            Guid iid = typeof(IPropertyStore).GUID;
            IPropertyStore store;
            Marshal.ThrowExceptionForHR(SHGetPropertyStoreForWindow(window, ref iid, out store));
            return store;
        }

        private static void SetString(IPropertyStore store, uint id, string text)
        {
            PropertyKey key = new PropertyKey(id);
            PropVariant value = new PropVariant();
            value.Type = 31; // VT_LPWSTR
            value.Text = Marshal.StringToCoTaskMemUni(text);
            try { Marshal.ThrowExceptionForHR(store.SetValue(ref key, ref value)); }
            finally { PropVariantClear(ref value); }
        }

        private static string QuotePath(string path)
        {
            // File paths cannot contain quotes; reject them rather than form an invalid command.
            if (String.IsNullOrEmpty(path) || path.IndexOf('"') >= 0)
                throw new ArgumentException("Invalid application path.");
            return "\"" + Path.GetFullPath(path) + "\"";
        }

        public static void ApplyWindow(IntPtr window, string launcherPath, string iconPath, string scriptPath)
        {
            string relaunchCommand;
            string iconResource;
            if (!String.IsNullOrEmpty(launcherPath) && File.Exists(launcherPath) &&
                String.Equals(Path.GetExtension(launcherPath), ".exe", StringComparison.OrdinalIgnoreCase))
            {
                relaunchCommand = QuotePath(launcherPath);
                iconResource = Path.GetFullPath(launcherPath) + ",0";
            }
            else
            {
                string powershell = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows),
                    "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
                relaunchCommand = QuotePath(powershell) + " -NoLogo -NoProfile -STA -WindowStyle Hidden -ExecutionPolicy Bypass -File " + QuotePath(scriptPath);
                iconResource = Path.GetFullPath(iconPath) + ",0";
            }
            IPropertyStore store = GetStore(window);
            try
            {
                // Set relaunch properties before the ID, while the HWND is not yet visible.
                SetString(store, 2, relaunchCommand);
                SetString(store, 3, iconResource);
                SetString(store, 4, "F1 Radio Translator");
                SetString(store, 5, ApplicationId);
                Marshal.ThrowExceptionForHR(store.Commit());
            }
            finally { Marshal.ReleaseComObject(store); }
        }

        public static void ClearWindow(IntPtr window)
        {
            if (window == IntPtr.Zero) return;
            IPropertyStore store = GetStore(window);
            try
            {
                foreach (uint id in new uint[] { 2, 3, 4, 5 })
                {
                    PropertyKey key = new PropertyKey(id);
                    PropVariant empty = new PropVariant(); // VT_EMPTY releases the Shell's stored value.
                    Marshal.ThrowExceptionForHR(store.SetValue(ref key, ref empty));
                }
                Marshal.ThrowExceptionForHR(store.Commit());
            }
            finally { Marshal.ReleaseComObject(store); }
        }
    }
}
