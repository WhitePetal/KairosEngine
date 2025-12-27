using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct RootSignatureDesc
	{
		public uint32 NumParameters;
		public RootParameter* pParameters;
		public uint32 NumStaticSamplers;
		public StaticSamplerDesc* pStaticSamplers;
		public RootSignatureFlags Flags;
	}
}