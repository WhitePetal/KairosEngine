#include "GraphicsPipelineState.h"

void KAIROS_API GraphicsPipelineState_Dispose(GraphicsPipelineState* _this)
{
    SAFE_RELEASE(_this->m_pPipelineState);
}
