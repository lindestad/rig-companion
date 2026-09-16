using System;
using System.Runtime.InteropServices;
public static class RigShortcut {
 [StructLayout(LayoutKind.Sequential)] public struct Key { public Guid fmtid; public uint pid; }
 [StructLayout(LayoutKind.Explicit, Size=24)] public struct Variant { [FieldOffset(0)] public ushort vt; [FieldOffset(8)] public IntPtr pointer; }
 [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"),InterfaceType(ComInterfaceType.InterfaceIsIUnknown)] interface Store {
  void GetCount(out uint count); void GetAt(uint index,out Key key); void GetValue(ref Key key,out Variant value); void SetValue(ref Key key,ref Variant value); void Commit();
 }
 [DllImport("shell32.dll",CharSet=CharSet.Unicode,PreserveSig=false)] static extern void SHGetPropertyStoreFromParsingName(string path,IntPtr context,uint flags,ref Guid iid,[MarshalAs(UnmanagedType.Interface)] out Store store);
 [DllImport("shell32.dll",CharSet=CharSet.Unicode)] static extern void SHChangeNotify(uint ev,uint flags,string path,IntPtr unused);
 [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
 static extern uint GetFinalPathNameByHandle(Microsoft.Win32.SafeHandles.SafeFileHandle handle, System.Text.StringBuilder path, uint capacity, uint flags);
 public static string PhysicalPath(string path) {
  using(var file=System.IO.File.OpenRead(path)) {
   var resolved=new System.Text.StringBuilder(32768);
   uint count=GetFinalPathNameByHandle(file.SafeFileHandle,resolved,(uint)resolved.Capacity,0);
   if(count==0 || count>=resolved.Capacity) throw new System.ComponentModel.Win32Exception();
   string result=resolved.ToString();
   return result.StartsWith(@"\\?\") ? result.Substring(4) : result;
  }
 }
 public static void Register(string path) {
  var iid=new Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"); Store store;
  SHGetPropertyStoreFromParsingName(path,IntPtr.Zero,2,ref iid,out store);
  var key=new Key{fmtid=new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"),pid=5};
  var value=new Variant{vt=31,pointer=Marshal.StringToCoTaskMemUni("RigCompanion.Desktop")};
  try{store.SetValue(ref key,ref value);store.Commit();}finally{Marshal.FreeCoTaskMem(value.pointer);Marshal.ReleaseComObject(store);}
  SHChangeNotify(0x00000002,0x1005,path,IntPtr.Zero);
  SHChangeNotify(0x00002000,0x1005,path,IntPtr.Zero);
  SHChangeNotify(0x00001000,5,System.IO.Path.GetDirectoryName(path),IntPtr.Zero);
 }
}
