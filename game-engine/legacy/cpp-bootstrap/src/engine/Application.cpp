#include "engine/Application.hpp"

#include <iostream>
#include <thread>

namespace engine {

Application::Application(std::string name)
    : m_name(std::move(name)) {}

void Application::run() {
    using clock = std::chrono::steady_clock;
    constexpr auto frameTime = std::chrono::milliseconds(16); // ~60 FPS

    std::cout << "[Engine] Starting '" << m_name << "'..." << std::endl;

    int frame = 0;
    auto lastTick = clock::now();

    while (m_running && frame < 300) { // demo loop: ~5 Sekunden
        const auto now = clock::now();
        const auto delta = std::chrono::duration_cast<std::chrono::milliseconds>(now - lastTick);
        lastTick = now;

        // Placeholder Update + Render
        std::cout << "[Frame " << frame << "] dt=" << delta.count() << "ms" << std::endl;

        std::this_thread::sleep_for(frameTime);
        ++frame;
    }

    std::cout << "[Engine] Shutdown." << std::endl;
}

} // namespace engine
