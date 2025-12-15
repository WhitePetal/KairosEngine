#ifndef __KAIROS_WINDOW__
#define __KAIROS_WINDOW__

#include "KairosEngineDefines.h"
#include <Windows.h>
#include <functional>

KAIROS_EXPORT_BEGIN

using WndProcCallback = std::function <LRESULT(HWND, UINT, WPARAM, LPARAM)> ;

HWND KAIROS_API KairosInitializeWindow(HINSTANCE hInstance, int ShowWnd, int width, int height, bool fullScreen, LPCWSTR windowName, LPCWSTR windowTitle, WndProcCallback* wndProc);

int KAIROS_API KairosDefWindowProcW(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam);

void KAIROS_API KairosDestroyWindow(HWND hWnd);

void KAIROS_API KairosPostQuitMessage(int nExitCode);

void KAIROS_API KairosMainLoop();

KAIROS_EXPORT_END

#endif // __KAIROS_WINDOW__

