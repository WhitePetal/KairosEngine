using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct DescriptorRange
	{
		public DescriptorRangeType RangeType;
		public uint32 NumDescriptors;
		public uint32 BaseShaderRegister;
		public uint32 RegisterSpace;
		public uint32 OffsetInDescriptorsFromTableStart;
	}
}