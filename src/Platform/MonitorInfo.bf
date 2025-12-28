using System;
using KairosEngine.Graphics;

namespace KairosEngine.Platform
{
	[CRepr]
	public struct MonitorInfo
	{
		public const int PRIMARY  =      0x00000001;

		public uint32 cbSize;
		public Rect rcMonitor;
		public Rect rcWork;
		public uint32 dwFlags;
	}
}