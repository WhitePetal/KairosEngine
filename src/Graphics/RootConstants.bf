using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct RootConstants
	{
		public uint32 ShaderRegister;
		public uint32 RegisterSpace;
		public uint32 Num32BitValues;
	}
}