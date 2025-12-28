using System;
using ImGui;

namespace KairosEngine.ImGUI
{
	public struct ImGuiWin32Data
	{
		public System.Windows.HWnd        	hWnd;
		public System.Windows.HWnd        	MouseHwnd;
		public int32                     	MouseTrackedArea;   // 0: not tracked, 1: client area, 2: non-client area
		public int32                     	MouseButtonsDown;
		public int64                       Time;
		public int64                       TicksPerSecond;
		public ImGui.MouseCursor        	LastMouseCursor;
		public uint32                      KeyboardCodePage;
		public bool                        WantUpdateMonitors;
	}
}