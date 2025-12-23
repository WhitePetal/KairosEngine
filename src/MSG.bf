using System;

namespace KairosEngine
{
	[CRepr]
	public struct MSGPOINT
	{
		public int64 x;
		public int64 y;
	}
	[CRepr]
	public struct MSG
	{
		public System.Windows.HWnd hwnd;
		public uint message;
		public uint64 wParam;
		public int64 lParam;
		public uint64 time;
		public MSGPOINT point;
	}
}