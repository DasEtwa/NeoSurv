#pragma once

#include <chrono>
#include <string>

namespace engine {

class Application {
public:
    explicit Application(std::string name);
    void run();

private:
    std::string m_name;
    bool m_running{true};
};

} // namespace engine
