using System;

namespace KairosEngine
{
	extension WindowSystem
	{
		private function int64(System.Windows.HWnd hwnd, uint32 mgs, uint64 wParam, int64 lParam) WndProcCallback = => WndProc;

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosInitializeWindow")]
		private static extern Windows.HWnd KairosInitializeWindow(Windows.HModule hInstance, int32 ShowWnd, int32 width, int32 height, bool fullScreen, char16* windowName, char16* windowTitle, function int64(System.Windows.HWnd hwnd, uint32 mgs, uint64 wParam, int64 lParam));

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosDefWindowProcW")]
		private static extern int32 KairosDefWindowProcW(Windows.HWnd hWnd, uint32 msg, uint64 wParam, int64 lParam);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosDestroyWindow")]
		private static extern void KairosDestroyWindow(Windows.HWnd hWnd);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosPostQuitMessage")]
		private static extern void KairosPostQuitMessage(int32 nExitCode);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosInitMSG")]
		private static extern void KairosInitMSG(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosPeekMessage")]
		private static extern int32 KairosPeekMessage(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosTranslateMessage")]
		private static extern int32 KairosTranslateMessage(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosDispatchMessage")]
		private static extern int64 KairosDispatchMessage(MSG* pMsg);
	}
}