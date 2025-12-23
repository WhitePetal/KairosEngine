namespace KairosEngine.Graphics
{
	public enum DescriptorHeapType : uint32
	{
		CBV_SRV_UAV	= 0,
		SAMPLER	= ( CBV_SRV_UAV + 1 ) ,
		RTV	= ( SAMPLER + 1 ) ,
		DSV	= ( RTV + 1 ) ,
		NUM_TYPES	= ( DSV + 1 ) 
	}
}