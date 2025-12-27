using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct DescriptorHeapDesc
	{
		public DescriptorHeapType Type;
		public uint32 NumDescriptors;
		public DescriptorHeapFlags Flags;
		public uint32 NodeMask;
	}
}