using System;

namespace KairosEngine
{
	extension WindowSystem
	{
		[CallingConvention(.Stdcall)]
		public delegate int WndProcDelegate(Windows.HWnd hwnd, uint msg, uint64 wParam, uint64 lParam);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosInitializeWindow")]
		public static extern Windows.HWnd KairosInitializeWindow(Windows.HModule hInstance, int ShowWnd, int width, int height, bool fullScreen, String windowName, String windowTitle, WndProcDelegate wndProc);
	}
}