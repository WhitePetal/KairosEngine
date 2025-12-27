namespace KairosEngine.Graphics
{
	public enum DescriptorRangeType : uint32
	{
		SRV	= 0,
		UAV	= ( SRV + 1 ) ,
		CBV	= ( UAV + 1 ) ,
		SAMPLER	= ( CBV + 1 ) 
	}
}