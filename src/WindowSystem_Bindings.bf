using System;

namespace KairosEngine
{
	extension WindowSystem
	{
		[CallingConvention(.Stdcall)]
		public delegate int64 WndProcPtr(Windows.HWnd hwnd, uint msg, uint64 wParam, int64 lParam);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosInitializeWindow")]
		public static extern Windows.HWnd KairosInitializeWindow(Windows.HModule hInstance, int ShowWnd, int width, int height, bool fullScreen, char16* windowName, char16* windowTitle, WndProcPtr wndProc);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosDefWindowProcW")]
		public static extern int KairosDefWindowProcW(Windows.HWnd hWnd, uint msg, uint64 wParam, int64 lParam);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosDestroyWindow")]
		public static extern void KairosDestroyWindow(Windows.HWnd hWnd);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosPostQuitMessage")]
		public static extern void KairosPostQuitMessage(int nExitCode);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosMainLoop")]
		public static extern void KairosMainLoop();
	}
}