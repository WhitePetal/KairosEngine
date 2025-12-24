using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct Rect
	{
		public int32 X;
		public int32 Y;
		public int32 Width;
		public int32 Height;

		public this(int32 x, int32 y, int32 w, int32 h)
		{
			X = x;
			Y = y;
			Width = w;
			Height = h;
		}
	}
}