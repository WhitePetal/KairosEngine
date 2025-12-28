namespace KairosEngine.Graphics
{
	public enum PipelineStateFlags : uint32
	{
		NONE	= 0,
		TOOL_DEBUG	= 0x1,
		DYNAMIC_DEPTH_BIAS	= 0x4,
		DYNAMIC_INDEX_BUFFER_STRIP_CUT	= 0x8
	}
}