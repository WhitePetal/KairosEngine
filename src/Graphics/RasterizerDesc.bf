using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct RasterizerDesc
	{
		public FillMode FillMode;
		public CullMode CullMode;
		public bool FrontCounterClockwise;
		public int32 DepthBias;
		public float DepthBiasClamp;
		public float SlopeScaledDepthBias;
		public bool DepthClipEnable;
		public bool MultisampleEnable;
		public bool AntialiasedLineEnable;
		public uint32 ForcedSampleCount;
		public ConservativeRasterizationMode ConservativeRaster;
	}
}