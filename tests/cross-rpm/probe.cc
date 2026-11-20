#include <iostream>
#include <numeric>
#include <vector>

int main() {
  const std::vector<int> values{10, 20, 12};
  const int sum = std::accumulate(values.begin(), values.end(), 0);
  std::cout << "C++ sum " << sum << ", pointer bits " << sizeof(void*) * 8
            << '\n';
  return sum == 42 ? 0 : 1;
}
