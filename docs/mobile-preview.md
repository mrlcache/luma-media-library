# Prévia mobile do Luma

A interface Svelte compartilhada pode ser testada em uma janela Tauri própria, com área inicial de 412 × 820 pixels lógicos. A janela pode ser redimensionada entre 360–480 pixels de largura e 640–920 de altura. Usa o mesmo Vite e hot reload do Luma DEV.

## Abrir

Na raiz do projeto, execute `npm run mobile:preview`. Na primeira vez, a sessão DEV precisa iniciar ou recompilar para detectar `.artifacts/mobile-preview.enabled`. Depois disso, a janela abre no início da sessão. O botão de celular ao lado do badge DEV na janela principal permite reabri-la.

A janela se identifica como **Luma Mobile · DEV** na barra nativa do Windows, sem cabeçalho ou controles de janela adicionais dentro do aplicativo. Ela mantém o modo mobile ao trocar de página, em armazenamento de sessão separado da janela desktop.

Para desativar a abertura automática, remova somente `.artifacts/mobile-preview.enabled` e feche a janela mobile. A versão instalada não abre essa janela; a criação automática e o comando de abertura são restritos a builds de desenvolvimento.

## Escopo da prévia no Windows

- Usa os dados reais e o backend do PC pela ponte Tauri existente, sem catálogo ou downloads simulados.
- Compartilha favoritos persistidos e os componentes de metadados, detalhes, episódios e releases.
- Usa uma composição própria para largura de celular, scroll nativo, navegação inferior e ações visíveis sem hover.
- A prévia não ativa Acrylic nem o Lenis desktop.

Esta é uma prévia da interface mobile em Windows, não um APK ou emulador Android. A reprodução usa a integração Windows; esta janela não comprova o funcionamento de recursos nativos Android, pareamento ou transmissão em um celular real.

Para gerar o APK, consulte [as instruções Android](android-demo.md). Para testar a interface em um celular com hot reload, consulte [o desenvolvimento por Wi-Fi](android-wifi-dev.md).
