project('slam', 'cpp', default_options: ['cpp_std=c++20'])


deps = [dependency('RTABMap')]
sources = [
  'src/main.cpp',
  'src/CameraRs2D4xx.cpp'
]

executable('slam', sources, dependencies: deps)
