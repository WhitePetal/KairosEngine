using System;

namespace KairosEngine.Platform
{
	public static class Kernel
	{
		typealias MONITORENUMPROC = function [CallingConvention(.Stdcall)] bool(
		    void* hMonitor,
		    void*      hdcMonitor,
		    void*    lprcMonitor, 
		    int64*   dwData
		);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosPostQuitMessage")]
		public static extern void PostQuitMessage(int32 nExitCode);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosInitMSG")]
		public static extern void InitMSG(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosPeekMessage")]
		public static extern int32 PeekMessage(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosTranslateMessage")]
		public static extern int32 TranslateMessage(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosDispatchMessage")]
		public static extern int64 DispatchMessage(MSG* pMsg);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosQueryPerformanceFrequency")]
		private static extern bool QueryPerformanceFrequency(int64* pFrequency);
		[Inline]
		public static bool QueryPerformanceFrequency(ref int64 frequency)
		{
			return QueryPerformanceFrequency(&frequency);
		}

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosQueryPerformanceCounter")]
		private static extern bool QueryPerformanceCounter(int64* pCounter);
		[Inline]
		public static bool QueryPerformanceCounter(ref int64 counter)
		{
			return QueryPerformanceCounter(&counter);
		}

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosGetANSI")]
		private static extern int GetKeyboardCodePgae(uint32* pKeyboardCodePage, int32 codePageSize);
		public static int GetKeyboardCodePgae(ref uint32 keyboardCodePage, int32 codePageSize)
		{
			return GetKeyboardCodePgae(&keyboardCodePage, codePageSize);
		}

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("KairosGetMonitorInfo")]
		public static extern bool GetMonitorInfo(void* monitor, MonitorInfo* pInfo);
	}
}