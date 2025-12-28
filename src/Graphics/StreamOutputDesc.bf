using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct StreamOutputDesc
	{
		public StreamOutputDeclartionEntry* pSODeclaration;
		public uint32 NumEntries;
		public uint32* pBufferStrides;
		public uint32 NumStrides;
		public uint32 RasterizedStream;
	}
}