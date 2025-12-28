using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct StreamOutputDeclartionEntry
	{
		public uint32 Stream;
		public char8* SemanticName;
		public uint32 SemanticIndex;
		public uint8 StartComponent;
		public uint8 ComponentCount;
		public uint8 OutputSlot;
	}
}