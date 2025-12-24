using System;

namespace KairosEngine
{
	public static class Kernel
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosPostQuitMessage")]
		public static extern void KairosPostQuitMessage(int32 nExitCode);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosInitMSG")]
		public static extern void KairosInitMSG(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosPeekMessage")]
		public static extern int32 KairosPeekMessage(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosTranslateMessage")]
		public static extern int32 KairosTranslateMessage(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosDispatchMessage")]
		public static extern int64 KairosDispatchMessage(MSG* pMsg);
	}
}