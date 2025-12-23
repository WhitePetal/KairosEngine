using System;

namespace KairosEngine
{
	extension WindowSystem
	{
		private function int64(System.Windows.HWnd hwnd, uint mgs, uint64 wParam, int64 lParam) WndProcCallback = => WndProc;

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosInitializeWindow")]
		private static extern Windows.HWnd KairosInitializeWindow(Windows.HModule hInstance, int ShowWnd, int width, int height, bool fullScreen, char16* windowName, char16* windowTitle, function int64(System.Windows.HWnd hwnd, uint mgs, uint64 wParam, int64 lParam));

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosDefWindowProcW")]
		private static extern int KairosDefWindowProcW(Windows.HWnd hWnd, uint msg, uint64 wParam, int64 lParam);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosDestroyWindow")]
		private static extern void KairosDestroyWindow(Windows.HWnd hWnd);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosPostQuitMessage")]
		private static extern void KairosPostQuitMessage(int nExitCode);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosInitMSG")]
		private static extern void KairosInitMSG(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosPeekMessage")]
		private static extern int KairosPeekMessage(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosTranslateMessage")]
		private static extern int KairosTranslateMessage(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosDispatchMessage")]
		private static extern int64 KairosDispatchMessage(MSG* pMsg);
	}
}