using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct StaticSamplerDesc
	{
		public SamplerFilter Filter;
		public TextureAddressMode AddressU;
		public TextureAddressMode AddressV;
		public TextureAddressMode AddressW;
		public float MipLODBias;
		public uint32 MaxAnisotropy;
		public ComparisonFunc ComparisonFunc;
		public StaticBorderColor BorderColor;
		public float MinLOD;
		public float MaxLOD;
		public uint32 ShaderRegister;
		public uint32 RegisterSpace;
		public ShaderVisibility ShaderVisibility;
	}
}