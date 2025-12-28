using System;
using KairosEngine.Platform;
using ImGui;
using static ImGui.ImGui;

namespace KairosEngine.ImGUI
{
	class ImGuiWin32Monitor
	{
		[CallingConvention(.Stdcall)]
		public static bool UpdateMonitorsEnumFunc(void* monitor, void* hdr, void* lprect, int64 lparam)
		{
			MonitorInfo info = MonitorInfo{};
			if(!Kernel.GetMonitorInfo(monitor, &info))
				return true;

			PlatformMonitor imgui_monitor = PlatformMonitor
			{
				MainPos = Vec2(info.rcMonitor.Left, info.rcMonitor.Top),
				MainSize = Vec2(info.rcMonitor.Width, info.rcMonitor.Height),
				WorkPos = Vec2(info.rcMonitor.Left, info.rcMonitor.Top),
				WorkSize = Vec2(info.rcMonitor.Width, info.rcMonitor.Height),
				DpiScale = GetDipScaleForMonitor(monitor),
				PlatformHandle = monitor
			};
			if(imgui_monitor.DpiScale <= 0f)
				return true;

			PlatformIO* io = GetPlatformIO();
			if(info.dwFlags & MonitorInfo.PRIMARY != 0)
				io.Monitors.PushFront(ref imgui_monitor);
			else
				io.Monitors.PushBack(ref imgui_monitor);
			return true;
		}

		private static float GetDipScaleForMonitor(void* monitor)
		{
			return 0f;
		}
	}
}