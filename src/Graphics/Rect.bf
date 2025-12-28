using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct Rect
	{
		public int32 Left;
		public int32 Top;
		public int32 Right;
		public int32 Bottom;

		public int32 Width
		{
			[Inline]
			get
			{
				return Right - Left;
			}
		}

		public int32 Height
		{
			[Inline]
			get
			{
				return Bottom - Top;
			}
		}

		public this(int32 left, int32 top, int32 right, int32 bottom)
		{
			Left = left;
			Top = top;
			Right = right;
			Bottom = bottom;
		}
	}
}