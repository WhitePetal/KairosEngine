using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct ViewPort
	{
		public float X;
		public float Y;
		public float Width;
		public float Height;
		public float MinDepth;
		public float MaxDepth;

		public this(float x, float y, float width, float height, float minD, float maxD)
		{
			X = x;
			Y = y;
			Width = width;
			Height = height;
			MinDepth = minD;
			MaxDepth = maxD;
		}
	}
}