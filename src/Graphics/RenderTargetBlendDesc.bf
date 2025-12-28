using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct RenderTargetBlendDesc
	{
		public bool BlendEnable;
		public bool LogicOpEnable;
		public Blend SrcBlend;
		public Blend DstBlend;
		public BlendOp BlendOp;
		public Blend SrcBlendAlpha;
		public Blend DstBlendAlpha;
		public BlendOp BlendOpAlpha;
		public LogicOp LogicOp;
		public uint8 RenderTargetWriteMask;

		/*[Inline]
		public this(bool blendEnable, Blend srcBlend, Blend dstBlend, BlendOp blendOp, Blend srcBlendAlpha, Blend dstBlendAlpha, BlendOp blendOpAlpha, uint8 renderTargetWriteMask)
		{
			BlendEnable = blendEnable;
			LogicOpEnable = false;
			SrcBlend = srcBlend;
			DstBlend = dstBlend;

		}*/
	}
}