using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct DescriptorGpuHandle
	{
		public uint64 Ptr;

		public this(uint64 ptr)
		{
			Ptr = ptr;
		}

		public void Offset(int32 offsetInDescriptors, uint32 descriptorIncrementSize) mut
		{
			Ptr = uint64(int64(Ptr) + int64(offsetInDescriptors) * int64(descriptorIncrementSize));
		}
	}
}