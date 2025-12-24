using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsVertexBufferView
	{
		public uint64 BufferLocation;
		public uint32 SizeInBytes;
		public uint32 StrideInBytes;

		public this(uint64 bufferLocation, uint32 sizeInBytes, int strideInBytes)
		{
			BufferLocation = bufferLocation;
			SizeInBytes = sizeInBytes;
			StrideInBytes = uint32(strideInBytes);
		}
	}
}