using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct RootDescriptorTable
	{
		public uint32 NumDescriptorRanges;
		public DescriptorRange* pDescriptorRanges;
	}
}