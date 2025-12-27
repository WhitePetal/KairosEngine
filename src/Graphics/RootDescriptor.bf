using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct RootDescriptor
	{
		public uint32 ShaderRegister;
		public uint32 RegisterSpace;
	}
}