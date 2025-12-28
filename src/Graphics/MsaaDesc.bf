using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct MsaaDesc
	{
		public uint32 Count;
		public uint32 Quality;
	}
}