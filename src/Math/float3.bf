using System;

namespace KairosEngine.Math
{
	[UnderlyingArray(typeof(float), 3, true)]
	public struct float3
	{
		public float x;
		public float y;
		public float z;

		[Inline]
		public this()
		{
			this = default;
		}

		[Inline]
		public this(float x, float y, float z)
		{
			this.x = x;
			this.y = y;
			this.z = z;
		}

		[Inline]
		public this(float scale)
		{
			this.x = this.y = this.z = scale;
		}

		public extern float this[int32 idx]
		{
			[Intrinsic("index")]
			get;
			[Intrinsic("index")]
			set;
		}

		[Intrinsic("add")]
		public static extern float3 operator+(float3 lhs, float3 rhs);
	}
}