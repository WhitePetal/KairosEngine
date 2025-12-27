using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct InputLayoutElement
	{
		public char8* SemanticName;
		public uint32 SemanticIndex;
		public InputLayoutElementFormat Format;
		public uint32 InputSlot;
		public uint32 AlignedByteOffset;
		public InputLayoutElementClass InputSlotClass;
		public uint32 InstanceDataStepRate;

		public this(String semanticeName, uint32 semanticIndex, InputLayoutElementFormat format, uint32 inputSlot, uint32 alignedByteOffset, InputLayoutElementClass inputSlotClass, uint32 instanceDataStepRate)
		{
			SemanticName = semanticeName.CStr();
			SemanticIndex = semanticIndex;
			Format = format;
			InputSlot = inputSlot;
			AlignedByteOffset = alignedByteOffset;
			InputSlotClass = inputSlotClass;
			InstanceDataStepRate = instanceDataStepRate;
		}
	}
}