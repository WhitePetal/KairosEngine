namespace KairosEngine.Graphics
{
	public enum ShaderCompileFlags : uint32
	{
		DEBUG                                = (1 << 0),
		SKIP_VALIDATION                      = (1 << 1),
		SKIP_OPTIMIZATION                    = (1 << 2),
		PACK_MATRIX_ROW_MAJOR                = (1 << 3),
		PACK_MATRIX_COLUMN_MAJOR             = (1 << 4),
		PARTIAL_PRECISION                    = (1 << 5),
		FORCE_VS_SOFTWARE_NO_OPT             = (1 << 6),
		FORCE_PS_SOFTWARE_NO_OPT             = (1 << 7),
		NO_PRESHADER                         = (1 << 8),
		AVOID_FLOW_CONTROL                   = (1 << 9),
		PREFER_FLOW_CONTROL                  = (1 << 10),
		ENABLE_STRICTNESS                    = (1 << 11),
		ENABLE_BACKWARDS_COMPATIBILITY       = (1 << 12),
		IEEE_STRICTNESS                      = (1 << 13),
		OPTIMIZATION_LEVEL0                  = (1 << 14),
		OPTIMIZATION_LEVEL1                  = 0,
		OPTIMIZATION_LEVEL2                  = ((1 << 14) | (1 << 15)),
		OPTIMIZATION_LEVEL3                  = (1 << 15),
		RESERVED16                           = (1 << 16),
		RESERVED17                           = (1 << 17),
		WARNINGS_ARE_ERRORS                  = (1 << 18),
		RESOURCES_MAY_ALIAS                  = (1 << 19),
		ENABLE_UNBOUNDED_DESCRIPTOR_TABLES   = (1 << 20),
		ALL_RESOURCES_BOUND                  = (1 << 21),
		DEBUG_NAME_FOR_SOURCE                = (1 << 22),
		DEBUG_NAME_FOR_BINARY                = (1 << 23),
	}
}