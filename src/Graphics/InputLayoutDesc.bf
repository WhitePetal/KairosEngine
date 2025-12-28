using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct InputLayoutDesc
	{
		public InputLayoutElementDesc* pInputElementDescs;
		public uint32 NumElements;

		public this(InputLayoutElementDesc[] inputElements, uint32 numElements)
		{
			pInputElementDescs = inputElements.Ptr;
			NumElements = numElements;
		}
	}
}