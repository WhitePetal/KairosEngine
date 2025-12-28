namespace KairosEngine.Graphics
{
	public enum DepthStencilOp : uint32
	{
		KEEP		= 1,
		ZERO		= 2,
		REPLACE		= 3,
		INCR_SAT	= 4,
		DECR_SAT	= 5,
		INVERT		= 6,
		INCR		= 7,
		DECR		= 8
	}
}