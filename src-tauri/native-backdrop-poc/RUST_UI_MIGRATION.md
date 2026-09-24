# Avaliação: migrar a UI para Rust nativo

Estado: análise inicial, 2026-09-23. Não altera a UI atual.

## Decisão curta

Não migrar a UI inteira agora. O backend e o core já são Rust; trocar Svelte
por uma UI Rust só ajuda a reduzir a memória do WebView se a nova UI renderizar
fora dele. Dioxus Desktop e Leptos dentro de Tauri continuam usando DOM/WebView,
portanto não resolvem esse objetivo por si sós. Dioxus tem um renderer nativo
GPU em caráter experimental, então também não é uma base de baixo risco para o
player neste momento.

Uma UI realmente nativa em Rust é possível com Slint ou Iced. Slint é o
candidato mais próximo para uma prova: UI declarativa, renderers com GPU e
fallback de software, backend desktop multiplataforma e guias atuais para
Android e iOS. A UI seria reescrita em `.slint`; não seria uma conversão de
Svelte/CSS para Rust automática. Iced também oferece renderização nativa via
wgpu e alternativa de software, mas teria de ser avaliado para os alvos móveis
pretendidos.

## O que a troca realmente muda

- Reutilizáveis: `media-core`, banco de dados, scanner, contratos Rust,
  comandos/domínio, modelos de biblioteca, lógica de progresso e assets.
- Reescrita manual: Home, sidebar, cartões, linhas horizontais, navegação,
  páginas de título, busca, configurações, estados responsivos, animações,
  scroll/virtualização, foco/teclado e controles do player. CSS, componentes
  Svelte e a estrutura HTML atual não migram como UI executável.
- Vídeo: Slint não fornece o mesmo caminho pronto de `<video>`/WebView2. Os
  exemplos oficiais de vídeo usam integrações específicas com FFmpeg ou
  GStreamer. Para um player real ainda precisamos ligar decodificação,
  superfícies GPU, áudio, legendas, HDR e DRM às APIs nativas de cada sistema.
  Esse trabalho também seria necessário numa UI Rust nativa de outra família.
- HSS/Acrylic: substituir o WebView não resolve automaticamente o backdrop.
  Uma janela Slint/Iced ainda precisaria da integração de composição específica
  do Windows. A trilha de WebView2 em composição, já investigada, mantém a UI
  atual e ataca diretamente esse problema.
- Memória: retirar o grupo de processos WebView2 pode reduzir o baseline; não
  há garantia sem medição. Renderers Skia/wgpu, posters decodificados, cache e
  superfícies de vídeo também consomem RAM/GPU. Compare idle, biblioteca grande,
  scroll e player, incluindo processos auxiliares e memória privada.

## Recomendação e critério para reabrir

Manter Tauri + Svelte enquanto resolvemos o HSS na mesma janela e medimos o
baseline real. Isso evita uma reescrita grande antes de sabermos se o WebView é
o gargalo percebido. Não usar Dioxus/Leptos como suposta otimização de memória
sem trocar o renderer WebView.

Se a memória medida continuar inaceitável depois de corrigir e perfilar a UI,
fazer uma prova isolada de uma única Home em Slint: poster rows virtualizadas,
scroll, input/foco, navegação por teclado, aparência aprovada e uma superfície
de vídeo nativa. Comparar a memória em idle e durante scroll/player, FPS,
acessibilidade via NVDA/VoiceOver, tempo de abertura e custo de manter o
backdrop. Só migrar tela a tela se isso provar uma vantagem material sem perder
o padrão visual. O backend Rust atual deve ficar compartilhado.

## Fontes primárias

- [Arquitetura Tauri](https://tauri.app/concept/architecture/) — Rust nativo
  para o host/backend; HTML/CSS em WebView para a interface.
- [Dioxus Desktop](https://github.com/DioxusLabs/dioxus/tree/main/packages/desktop)
  e [Dioxus](https://github.com/DioxusLabs/dioxus) — Desktop WebView e estado
  experimental dos renderers nativos.
- [Slint backends/renderers](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/)
  e [Winit backend](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/)
  — renderização GPU/software e suporte desktop.
- [Slint Android](https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/android/)
  e [Slint iOS](https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/ios/)
  — requisitos e configuração por plataforma.
- [Exemplos Slint](https://github.com/slint-ui/slint/blob/master/examples/README.md)
  — exemplos de vídeo com GStreamer/FFmpeg; integração, não elemento de vídeo
  pronto equivalente ao browser.
- [Iced wgpu renderer](https://docs.rs/iced_wgpu/latest/iced_wgpu/) — renderer
  GPU e primitives suportadas.
