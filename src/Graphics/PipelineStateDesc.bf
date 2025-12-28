using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct CachedPipelineState
	{
		public void* pCachedBlob;
		public uint64 CachedBlobSizeInBytes;
	}

	[CRepr]
	public struct PipelineStateDesc
	{
		public void* pRootSignature;
		public ShaderByteCode VS;
		public ShaderByteCode PS;
		public ShaderByteCode DS;
		public ShaderByteCode HS;
		public ShaderByteCode GS;
		public StreamOutputDesc StreamOutput;
		public BlendDesc BlendState;
		public uint32 SampleMask;
		public RasterizerDesc RasterizerState;
		public DepthStencilDesc DepthStencilState;
		public InputLayoutDesc InputLayout;
		public IndexBufferStripCutValue IBStripCutValue;
		public PrimitiveTopologyType PrimitiveTopologyType;
		public uint32 NumRenderTargets;
		public RenderTargetFormat[8] RTVFormats;
		public DepthStencilFormat DSVFormat;
		public MsaaDesc AADesc;
		public uint32 NodeMask;
		public CachedPipelineState CachedPSO;
		public PipelineStateFlags Flags;

		public this(GraphicsRootSignature rootSignature, ref BlendDesc blendState, ref RasterizerDesc rasterizerState, ref DepthStencilDesc depthStencilState, ref InputLayoutDesc inputLayout,
			ref MsaaDesc aaDesc, ref RenderTargetFormat rtvFormat, uint32 numRenderTargets, DepthStencilFormat dsvFormat, PrimitiveTopologyType topologyType, PipelineStateFlags flags,
			uint32 sampleMask, uint32 nodeMask, GraphicsShader vs, GraphicsShader ps)
		{
			pRootSignature = rootSignature.pInternalRootSignature;
			VS = ShaderByteCode(vs);
			PS = ShaderByteCode(ps);
			DS = default;
			HS = default;
			GS = default;
			StreamOutput = default;
			BlendState = blendState;
			SampleMask = sampleMask;
			RasterizerState = rasterizerState;
			DepthStencilState = depthStencilState;
			InputLayout = inputLayout;
			IBStripCutValue = IndexBufferStripCutValue.DISABLED;
			PrimitiveTopologyType = topologyType;
			NumRenderTargets = numRenderTargets;
			RTVFormats[0] = rtvFormat;
			RenderTargetFormat* pRtvFormat = &rtvFormat;
			for(uint32 i = 0; i < NumRenderTargets; ++i)
				RTVFormats[i] = pRtvFormat[i];
			DSVFormat = dsvFormat;
			AADesc = aaDesc;
			NodeMask = nodeMask;
			CachedPSO = default;
			Flags = flags;
		}
	}
}