# Universal --- Visão de Produto e Especificação Inicial

> Documento-base para desenvolver o projeto com Claude Code.\
> Nome provisório: **Universal** (pode mudar).\
> Tipo: plataforma open-source e extensível para gerenciamento de
> dispositivos, ambientes computacionais, infraestrutura e saúde de
> aplicações.

------------------------------------------------------------------------

## 1. Resumo executivo

O Universal é uma plataforma para **conhecer, configurar, padronizar,
monitorar e administrar computadores, servidores, VPS, ambientes de
desenvolvimento e aplicações** em um único lugar.

A proposta combina quatro necessidades que normalmente ficam espalhadas
em diversos programas:

1.  **Device Management:** inventário, monitoramento, diagnóstico,
    benchmarks, otimização e periféricos.
2.  **Environment Management:** presets e sincronização de ambientes,
    aplicativos, aparência, configurações e ferramentas entre
    computadores.
3.  **Infrastructure & Project Health:** monitoramento de servidores,
    VPS, containers, serviços, APIs, bancos de dados e projetos.
4.  **Fleet & Organization Management:** gerenciamento de vários
    dispositivos e usuários, com políticas, permissões, auditoria e
    administração interna para empresas.

O produto deve funcionar tanto para uma pessoa que quer manter seus
computadores iguais quanto para uma empresa que precisa administrar
estações de trabalho, servidores e aplicações.

**Princípios fundamentais:**

-   Local-first: funções locais essenciais continuam funcionando sem
    internet.
-   Self-hostable: o Core pode ser instalado em uma VPS, servidor
    próprio ou homelab.
-   Cloud-ready: futuramente pode existir uma oferta hospedada.
-   Cross-platform: arquitetura preparada para Windows, Linux e macOS,
    respeitando as diferenças de cada sistema.
-   Modular: módulos e adaptadores independentes.
-   API-first: funcionalidades acessíveis por APIs documentadas.
-   Secure-by-default: privilégios mínimos, aprovação de ações
    sensíveis, auditoria e reversão.
-   Extensível: plugins, integrações e adaptadores comunitários ou
    empresariais.
-   Vendor-neutral: não depender de um único fabricante, distribuidor
    Linux ou provedor de nuvem.

O Universal **não deve depender de um agente de IA** para funcionar. A
IA é uma integração opcional que pode consultar dados e solicitar ações
por interfaces controladas.

------------------------------------------------------------------------

## 2. Problema que o produto resolve

### 2.1 Uso pessoal e múltiplos computadores

Usuários que trabalham em mais de um computador precisam repetir
configurações manualmente:

-   Instalar os mesmos aplicativos.
-   Recriar configurações de terminal, editores e ferramentas.
-   Replicar aparência, temas, fontes e preferências.
-   Manter pastas de projetos em locais previsíveis.
-   Sincronizar repositórios Git sem perder alterações locais.
-   Atualizar aplicativos, bibliotecas, sistema e drivers.
-   Reconfigurar periféricos e preferências de hardware.
-   Descobrir por que uma máquina está diferente ou com desempenho pior
    que outra.

O Universal deve transformar a configuração de um computador em um
**ambiente declarativo e reproduzível**, que possa ser aplicado a outros
dispositivos compatíveis.

### 2.2 Administração de infraestrutura

Acompanhar computadores, VPS, servidores, homelabs, containers e
aplicações exige várias ferramentas diferentes. O Universal deve
oferecer uma visão integrada de:

-   Saúde e disponibilidade de máquinas.
-   CPU, memória, armazenamento, rede e sensores.
-   Serviços e containers.
-   APIs, requisições, latência e erros.
-   Histórico de deploys e incidentes.
-   Alertas e tendências.
-   Relação entre problemas de uma aplicação e problemas da
    infraestrutura que a executa.

### 2.3 Uso empresarial

Empresas precisam preparar, padronizar e manter muitos dispositivos e
serviços:

-   Onboarding e offboarding de funcionários.
-   Aplicação de configurações aprovadas.
-   Inventário de hardware e software.
-   Gestão de atualizações e vulnerabilidades.
-   Políticas diferentes por departamento, função ou grupo.
-   Monitoramento de servidores e aplicações internas.
-   Auditoria de alterações e ações administrativas.
-   Visibilidade sobre conformidade e estado da frota.

O Universal deve permitir que a mesma plataforma seja usada por uma
pessoa, uma família, uma equipe ou uma organização com centenas ou
milhares de dispositivos, respeitando limites de escala e arquitetura.

------------------------------------------------------------------------

## 3. Visão do produto e experiência principal

A experiência ideal:

1.  O usuário instala o Universal em um computador ou servidor.
2.  Faz login ou conecta o dispositivo a uma instância self-hosted.
3.  O Universal identifica o sistema, hardware, softwares e capacidades
    disponíveis.
4.  O usuário escolhe um espaço pessoal ou organização.
5.  Pode selecionar um preset existente ou criar um novo.
6.  O Universal compara o estado atual com o estado desejado.
7.  Apresenta um plano de mudanças, com explicações e riscos.
8.  O usuário aprova as alterações.
9.  O dispositivo passa a ser monitorado e administrado conforme suas
    permissões.

Ao configurar um computador novo, o objetivo é restaurar rapidamente o
ambiente de trabalho: aplicativos, preferências, ferramentas, pastas,
repositórios e configurações compatíveis.

O Universal deve distinguir claramente:

-   **O que é desejado:** definido pelo usuário ou política da
    organização.
-   **O que existe atualmente:** estado detectado no dispositivo.
-   **O que pode ser feito:** ações suportadas naquele sistema.
-   **O que foi aplicado:** histórico verificável de mudanças.

------------------------------------------------------------------------

## 4. Personas e espaços de trabalho

### 4.1 Pessoa individual

-   Administra seus próprios computadores, notebooks, servidores e VPS.
-   Cria presets pessoais.
-   Sincroniza aplicativos, configurações e projetos.
-   Usa monitoramento e alertas.

### 4.2 Família ou pequeno grupo

-   Compartilha alguns dispositivos.
-   Mantém espaços e dados separados por pessoa.
-   Concede acesso limitado a familiares.
-   Pode administrar computadores de forma assistida.

### 4.3 Desenvolvedor ou equipe técnica

-   Padroniza ambientes de desenvolvimento.
-   Mantém repositórios em várias máquinas.
-   Monitora aplicações, APIs, bancos e infraestrutura.
-   Acompanha deploys e incidentes.

### 4.4 Empresa / TI

-   Gerencia uma frota de endpoints.
-   Cria políticas por grupos, departamentos, cargos e locais.
-   Distribui configurações e aplicativos aprovados.
-   Controla atualizações e inventário.
-   Gerencia permissões, auditoria e acesso remoto autorizado.
-   Integra identidade corporativa e ferramentas existentes.

### 4.5 Provedor ou equipe de operações

-   Acompanha múltiplos ambientes, servidores e clientes.
-   Organiza infraestrutura por organização, projeto e ambiente.
-   Recebe alertas e relatórios.
-   Pode futuramente operar como MSP (provedor de serviços gerenciados),
    com isolamento rigoroso entre clientes.

------------------------------------------------------------------------

## 5. Estrutura conceitual: usuários, organizações e dispositivos

Modelo conceitual sugerido:

-   **User:** identidade de uma pessoa.
-   **Organization:** espaço pessoal, familiar, equipe ou empresa.
-   **Membership:** vínculo de usuário com organização e papel
    atribuído.
-   **Group:** agrupamento de dispositivos ou usuários.
-   **Device:** computador, notebook, workstation, servidor ou VPS.
-   **Agent:** processo instalado no dispositivo e responsável por
    coletar dados e executar operações autorizadas.
-   **Project:** aplicação ou projeto monitorado.
-   **Environment:** ambiente de aplicação, como produção, staging e
    desenvolvimento.
-   **Service:** API, worker, banco, container ou serviço relacionado a
    um projeto.
-   **Preset:** estado desejado de configurações, aplicativos e
    ferramentas.
-   **Policy:** regra obrigatória ou recomendada para um grupo de
    dispositivos.
-   **Integration:** conexão com serviço externo.
-   **Alert:** evento que exige atenção.
-   **AuditEvent:** registro imutável ou append-only de ações
    relevantes.

Um usuário pode pertencer a várias organizações. Um dispositivo pertence
a uma organização por vez no modelo inicial, mas o desenho deve permitir
transferência controlada, compartilhamento delegado ou gestão por
prestador no futuro.

### 5.1 Papéis iniciais

-   Owner: controle total da organização.
-   Admin: administração de usuários, grupos, dispositivos e políticas.
-   Operator: pode executar operações permitidas.
-   Developer: acesso a projetos, ambientes e métricas autorizadas.
-   Viewer: somente leitura.
-   Auditor: leitura de inventário, políticas e trilha de auditoria, sem
    executar mudanças.

Permissões devem ser granulares e avaliadas no servidor e no agente. A
interface não pode ser a única barreira de segurança.

------------------------------------------------------------------------

## 6. Módulos do produto

## 6.1 Dashboard

Visão agregada e personalizável:

-   Dispositivos online/offline.
-   Servidores e VPS.
-   Saúde de aplicações e projetos.
-   Alertas recentes.
-   Atualizações pendentes.
-   Uso de recursos.
-   Incidentes ativos.
-   Tarefas em execução.
-   Estado de conformidade das políticas.

O dashboard deve permitir filtros por organização, grupo, dispositivo,
projeto, ambiente e período.

## 6.2 Device Management

Inventário e monitoramento de computadores:

-   CPU, modelo, núcleos, threads, clocks e utilização.
-   GPU, modelo, memória e utilização, quando disponível.
-   RAM total, uso e pressão de memória.
-   Placa-mãe, BIOS/UEFI e firmware, quando disponível.
-   Armazenamento, espaço, SMART/saúde quando suportado.
-   Temperaturas, ventoinhas, clocks, potência e consumo, quando
    sensores e permissões permitirem.
-   Sistema operacional, versão, kernel e arquitetura.
-   Drivers e dispositivos conectados.
-   Processos, serviços e inicialização.
-   Rede, interfaces, tráfego e conectividade.
-   Histórico de alterações de hardware e software.

A coleta deve ser configurável, com frequência ajustável para evitar
consumo desnecessário de recursos.

### Diagnóstico e benchmark

-   Testes de CPU.
-   Testes de GPU.
-   Testes de memória.
-   Testes de armazenamento.
-   Testes de rede.
-   Testes de estabilidade e stress.
-   Monitoramento térmico durante testes.
-   Histórico e comparação entre máquinas.

Testes potencialmente intensivos ou arriscados devem exigir
consentimento explícito, avisar sobre temperatura, consumo e duração, e
permitir cancelamento. Não executar testes destrutivos por padrão.

Resultados devem registrar hardware, versão da ferramenta, duração,
condições e sistema operacional. Comparações devem considerar diferenças
de hardware e não prometer ganhos de desempenho.

### Otimização

-   Perfis de energia.
-   Inicialização de aplicativos.
-   Configurações do sistema.
-   Recomendações por jogo.
-   Latência e desempenho.
-   Configurações de GPU, quando suportadas.
-   Curvas de ventoinhas, quando suportadas.
-   Overclock e undervolt somente em módulos futuros, com validações
    rigorosas.

O Universal deve explicar cada alteração, medir quando possível,
oferecer backup/reversão e evitar scripts genéricos de "otimização
milagrosa".

## 6.3 Peripheral Management

Gerenciamento unificado de periféricos:

-   Mouse, teclado, headset, microfone e webcam.
-   Monitores e configurações de tela.
-   DPI, polling rate, perfis e iluminação RGB, quando suportados.
-   Configurações específicas por aplicativo ou jogo.
-   Perfis por dispositivo e sincronização seletiva.

Não prometer suporte universal: protocolos, APIs e recursos variam por
fabricante. Criar adaptadores por fabricante/modelo e aproveitar
projetos existentes quando as licenças permitirem.

## 6.4 Environment Management

Módulo central para padronizar computadores.

### Presets declarativos

Um preset representa o ambiente desejado, não uma imagem completa do
computador.

Pode conter:

-   Aplicativos obrigatórios, opcionais e exclusivos.
-   Configurações de sistema.
-   Aparência, tema, fontes e papel de parede.
-   Terminal, shell, aliases e dotfiles.
-   Configurações de editores e IDEs.
-   Ferramentas de desenvolvimento.
-   Configurações Git.
-   Ferramentas e extensões de IA/coding assistants.
-   Estrutura de pastas.
-   Repositórios Git.
-   Variáveis não sensíveis.
-   Preferências de periféricos.
-   Configurações específicas por sistema operacional.

### Categorias de aplicativos

-   **Obrigatório:** deve estar instalado nos dispositivos compatíveis
    que recebem o preset.
-   **Opcional:** fica disponível para instalação, mas não é instalado
    automaticamente.
-   **Exclusivo:** pertence a um dispositivo ou grupo específico.
-   **Bloqueado / não permitido:** em contexto empresarial, não deve
    estar instalado ou executado conforme a política aplicável.

### Detecção de novos aplicativos

Quando um aplicativo for instalado, o Universal pode perguntar:

-   Adicionar ao preset compartilhado.
-   Manter somente neste dispositivo.
-   Marcar como opcional.
-   Ignorar ou perguntar depois.

A detecção precisa evitar prompts excessivos. Deve reconhecer
instalações feitas pelo próprio Universal e permitir regras por
categoria, origem ou pacote.

### Estado desejado e reconciliação

Criar um Configuration Engine que compare:

-   Desired State: o que o preset ou política determina.
-   Actual State: o que foi detectado.
-   Capability: o que o sistema e o dispositivo suportam.
-   Plan: mudanças necessárias.
-   Execution: aplicação das mudanças aprovadas.
-   Drift: divergência entre estado desejado e atual.

O motor deve ser idempotente: executar novamente uma configuração já
aplicada não deve causar efeitos colaterais ou duplicações.

Antes de executar, mostrar um plano legível com alterações,
dependências, riscos e necessidade de reinicialização. Permitir aprovar
tudo, aprovar por item ou ignorar.

### Exceções por dispositivo

Nem todos os computadores são iguais. Permitir:

-   Exceções locais.
-   Exceções por grupo.
-   Valores específicos por dispositivo.
-   Recursos condicionais por sistema operacional.
-   Regras por arquitetura e hardware.
-   Configurações não aplicáveis com justificativa.

Não forçar configurações de desktop em notebooks ou servidores.

## 6.5 Application Management

Gerenciar instalação, remoção e identificação de aplicativos.

Adaptadores planejados:

  Plataforma        Gerenciadores / fontes
  ----------------- -------------------------------------------
  Arch Linux        pacman, AUR helpers, Flatpak
  Debian / Ubuntu   APT, Flatpak, Snap
  Fedora            DNF, Flatpak
  openSUSE          Zypper, Flatpak
  Windows           WinGet, Windows Update, Chocolatey, Scoop
  macOS             Homebrew, Mac App Store, Software Update

A implementação deve priorizar fontes oficiais e adaptadores com
contratos bem definidos. Não executar instaladores desconhecidos sem
confirmação.

Um aplicativo pode ter diferentes identificadores de pacote em cada
plataforma. O catálogo deve mapear uma identidade lógica do aplicativo
para os pacotes disponíveis por sistema, arquitetura, versão e canal.

## 6.6 Workspace & Git Management

Módulo especialmente importante para desenvolvedores que trabalham em
vários computadores.

### Onboarding de workspace

-   Detectar ou solicitar o diretório padrão de projetos.
-   Exemplo de caminho: `~/Documents/workspace/`.
-   No Windows, oferecer um caminho equivalente configurável.
-   Identificar repositórios Git dentro de diretórios selecionados.
-   Permitir cadastrar repositórios manualmente.
-   Associar repositórios a projetos, presets e dispositivos.
-   Não presumir que todos os diretórios encontrados devam ser
    sincronizados.

### Visão centralizada

Para cada repositório, mostrar:

-   Nome e URL remota.
-   Branch atual.
-   Último commit local e remoto.
-   Alterações locais não commitadas.
-   Commits locais não enviados.
-   Commits remotos pendentes.
-   Divergência de branches.
-   Estado da última sincronização.
-   Caminho local em cada dispositivo.

### Sincronização segura

A ação "Atualizar todos" deve primeiro fazer uma fase de inspeção:

1.  Verificar se o diretório existe.
2.  Verificar se é um repositório válido.
3.  Identificar branch e remoto.
4.  Executar fetch.
5.  Detectar alterações locais, commits não enviados e divergências.
6.  Classificar se o pull é seguro.
7.  Apresentar o plano.
8.  Executar somente as atualizações aprovadas.

Nunca executar `git pull` cegamente em todos os diretórios. Nunca
sobrescrever alterações locais, fazer reset destrutivo, trocar branches
ou resolver conflitos automaticamente sem política e aprovação
explícitas.

Permitir sincronização por grupo, projeto, dispositivo ou repositório
individual.

### Integração com GitHub/GitLab

Futuramente: - Importar lista de repositórios autorizados. - Detectar
novos repositórios. - Mostrar estado de branches e workflows. - Associar
commits e deploys à saúde do projeto. - Usar OAuth ou tokens com escopo
mínimo.

Não armazenar tokens em texto puro.

## 6.7 Updates & Maintenance

Uma central para identificar atualizações em computadores e servidores.

Categorias:

-   Aplicativos.
-   Pacotes e bibliotecas do sistema.
-   Sistema operacional e kernel.
-   Ferramentas de desenvolvimento.
-   Drivers.
-   Firmware.
-   Atualizações de segurança.

A tela deve mostrar: - Atualizações disponíveis. - Origem e versão
atual/nova. - Prioridade ou severidade, quando conhecida. -
Dependências. - Tamanho, quando disponível. - Necessidade de
reinicialização. - Riscos ou incompatibilidades conhecidas. - Estado da
instalação.

Ações: - Verificar atualizações. - Atualizar item selecionado. -
Atualizar categoria. - Atualizar dispositivo. - Agendar atualização. -
Adiar ou ignorar conforme política. - Consultar histórico.

### Segurança de atualizações

-   Não instalar drivers ou firmware indiscriminadamente.
-   Verificar origem, assinatura e compatibilidade quando possível.
-   Diferenciar atualização disponível de atualização recomendada.
-   Em empresas, suportar anéis de atualização: teste, piloto e
    produção.
-   Permitir janelas de manutenção e reinicializações controladas.
-   Registrar resultado e falhas.
-   Nunca reiniciar sem política ou consentimento apropriado.

## 6.8 Infrastructure Management

Para VPS, servidores, homelabs e infraestrutura de nuvem:

-   CPU, RAM, disco e rede.
-   Uptime e disponibilidade.
-   Processos e serviços.
-   Containers e imagens Docker.
-   Volumes e uso de armazenamento.
-   Logs e eventos.
-   Serviços systemd e equivalentes.
-   Conectividade e portas monitoradas.
-   Certificados e expiração, quando integrados.
-   Backups e resultados de tarefas, quando houver integração.
-   Proxmox e virtualização.
-   Recursos de provedores de VPS por integração.

O agente deve funcionar em Linux e ser leve. Monitoramento básico deve
funcionar sem acesso privilegiado sempre que possível.

## 6.9 Project Health & Application Observability

O Universal deve monitorar a saúde de aplicações, APIs e projetos, não
apenas a máquina em que estão hospedados.

### Métricas

-   Uptime e disponibilidade.
-   Latência, incluindo percentis quando possível.
-   Requisições por período.
-   Taxas de erro HTTP 4xx e 5xx.
-   Exceções e erros internos.
-   Logs estruturados.
-   Traces distribuídos.
-   Filas e workers.
-   Conexões, latência e erros de banco de dados.
-   Dependências externas.
-   Deploys e versões.
-   Consumo de recursos por serviço, quando instrumentado.

### Integrações

Preferir padrões existentes: - OpenTelemetry para métricas, traces e
logs. - Prometheus para coleta e consulta de métricas. - Exporters e
agentes existentes quando apropriado. - Health checks HTTP/TCP. -
Webhooks para eventos de deploy e incidentes.

O Universal não deve exigir SDK proprietário para monitorar uma
aplicação. Deve aceitar padrões abertos e integrações externas.

### Correlação

Uma das funcionalidades de maior valor será relacionar eventos:

Exemplo: - A latência de uma API aumenta. - O uso de CPU da VPS sobe. -
O banco apresenta aumento de conexões. - O sistema apresenta a linha do
tempo correlacionada e sugere possíveis causas, sem afirmar causalidade
sem evidência.

Permitir associar: - Organização. - Projeto. - Ambiente (produção,
staging, desenvolvimento). - Serviço. - Repositório. - Deploy. -
Servidor ou container que executa o serviço.

## 6.10 Automation

Motor de tarefas e políticas:

-   Verificar atualizações.
-   Sincronizar repositórios.
-   Aplicar presets.
-   Executar diagnósticos.
-   Criar relatórios.
-   Enviar alertas.
-   Executar tarefas de manutenção.
-   Reagir a eventos e limites.

Automações devem ter escopo, gatilho, permissões, limites, histórico e
opção de desativação. Ações destrutivas exigem aprovação explícita ou
política empresarial muito bem definida.

## 6.11 Integrations

Integrações opcionais e desacopladas:

-   TruthID.
-   Agente de IA próprio ou de terceiros.
-   GitHub e GitLab.
-   Docker.
-   Proxmox.
-   Tailscale / WireGuard.
-   OpenTelemetry / Prometheus.
-   Provedores de VPS e cloud.
-   Sistemas de identidade empresarial.
-   Ferramentas de alertas e comunicação.

O Universal deve funcionar sem TruthID e sem IA. TruthID pode fornecer
autenticação/identidade, mas autorização, papéis, sessões, revogação de
dispositivos e políticas continuam sendo responsabilidades explícitas do
Universal.

------------------------------------------------------------------------

## 7. Integração opcional com agente de IA

O agente de IA é um projeto independente, não o núcleo do Universal.

O Universal expõe APIs e ferramentas documentadas para que um agente
autorizado possa:

-   Consultar inventário e métricas.
-   Investigar alertas.
-   Comparar dispositivos.
-   Solicitar diagnósticos.
-   Analisar saúde de aplicações.
-   Consultar estado de presets e atualizações.
-   Propor mudanças.
-   Solicitar execução de tarefas permitidas.

### Modelo de segurança

A IA não recebe shell irrestrito nem acesso administrativo geral.

Ela utiliza ferramentas específicas, com: - Escopos por organização e
dispositivo. - Permissões de leitura e escrita separadas. - Aprovação
humana para ações sensíveis. - Limites de execução e timeout. - Registro
de todas as ações. - Proteção contra prompt injection em logs, nomes de
processos, repositórios e conteúdo externo. - Possibilidade de revogar a
integração.

A IA deve distinguir observação, hipótese e diagnóstico confirmado.

------------------------------------------------------------------------

## 8. Arquitetura técnica proposta

A stack abaixo é uma hipótese inicial, a ser validada com protótipos e
pesquisa de bibliotecas existentes.

### 8.1 Aplicações

-   **Desktop:** Tauri + React + TypeScript.
-   **Web:** React + TypeScript.
-   **Agentes:** Rust.
-   **Core/API:** Rust, avaliando framework maduro e ecossistema.
-   **Banco transacional:** PostgreSQL.
-   **Métricas:** avaliar TimescaleDB, Prometheus e/ou armazenamento
    especializado conforme volume e retenção.
-   **Comunicação:** HTTPS e WebSocket inicialmente; gRPC pode ser usado
    internamente ou em comunicação agente-Core quando justificar.
-   **Observabilidade interna:** OpenTelemetry.
-   **Containers:** Docker para distribuição self-hosted.

Não duplicar serviços sem necessidade. Começar com um monólito modular
para o Core, separando processos apenas quando escala, isolamento ou
operação justificarem.

### 8.2 Componentes

1.  **Universal Core**
    -   API.
    -   Organizações e usuários.
    -   Autorização.
    -   Cadastro e revogação de dispositivos.
    -   Inventário.
    -   Presets e políticas.
    -   Tarefas.
    -   Alertas.
    -   Histórico.
    -   Integrações.
    -   Auditoria.
2.  **Universal Agent**
    -   Coleta local.
    -   Detecção de capacidades.
    -   Execução de tarefas permitidas.
    -   Aplicação de presets.
    -   Atualizações.
    -   Comunicação segura com Core.
    -   Buffer local durante desconexão.
3.  **Universal Desktop**
    -   Interface local.
    -   Monitoramento em tempo real.
    -   Configuração do dispositivo.
    -   Diagnósticos.
    -   Periféricos.
    -   Acesso ao Core remoto ou local.
4.  **Universal Web**
    -   Administração centralizada.
    -   Gestão de organizações.
    -   Frota de dispositivos.
    -   Infraestrutura e projetos.
    -   Políticas e relatórios.
5.  **Plugins e adaptadores**
    -   Pacotes, sensores, periféricos, provedores, sistemas e
        integrações.

### 8.3 Local-first e desconexão

-   O agente deve manter funções locais essenciais sem conexão.
-   Presets e políticas previamente sincronizados podem continuar
    disponíveis.
-   Métricas podem ser armazenadas em buffer local com limite de tamanho
    e retenção.
-   Tarefas remotas devem expirar e não executar depois de perder
    validade.
-   Ao reconectar, sincronizar com controle de versão e resolução
    explícita de conflitos.
-   O estado do dispositivo deve indicar quando os dados foram
    atualizados pela última vez.

### 8.4 Comunicação e rede

-   Agente inicia conexão de saída com o Core por padrão.
-   Evitar exigir portas administrativas abertas nos computadores.
-   Autenticação mútua ou credenciais de dispositivo com rotação e
    revogação.
-   TLS em trânsito.
-   Tailscale/WireGuard podem ser integrações opcionais, não
    dependências obrigatórias.
-   P2P pode ser avaliado futuramente; não é requisito do MVP.
-   Relay seguro pode ser necessário para acesso remoto em redes
    restritas.

### 8.5 Multi-tenancy

O Core deve separar dados por organização desde o início.

-   Toda entidade pertencente a uma organização carrega o escopo
    correspondente.
-   Toda consulta e operação verifica autorização no servidor.
-   Isolamento deve ser testado, não apenas presumido.
-   Segredos e credenciais devem ter proteção própria.
-   A arquitetura deve suportar uma instalação self-hosted para uma
    organização e, futuramente, uma instalação multi-tenant hospedada.

Não criar microserviços prematuramente. Um monólito modular com limites
claros é mais simples para o início.

------------------------------------------------------------------------

## 9. Uso empresarial: como funcionaria

A versão empresarial não deve ser apenas a versão pessoal com mais
dispositivos. Ela precisa oferecer governança, segurança, operação e
controle.

### 9.1 Onboarding de empresa

1.  Administrador cria ou recebe uma organização.
2.  Configura domínio e identidade corporativa, quando houver
    integração.
3.  Cria grupos: departamentos, equipes, locais ou funções.
4.  Define papéis e permissões.
5.  Cria presets e políticas aprovadas.
6.  Distribui instalador ou método de implantação do agente.
7.  Dispositivos se registram com processo de aprovação ou enrollment
    controlado.
8.  O Universal inventaria os dispositivos e compara com políticas.
9.  TI acompanha conformidade, atualizações e alertas.

### 9.2 Políticas por grupo

Exemplos: - Desenvolvedores recebem IDEs, Git, ferramentas e
configurações de desenvolvimento. - Design recebe softwares e fontes
aprovados. - Financeiro recebe um conjunto restrito de aplicativos. -
Servidores recebem políticas de segurança e manutenção específicas. -
Notebooks recebem configurações de energia diferentes de desktops.

Políticas podem ser: - Obrigatórias. - Recomendadas. - Informativas. -
Proibidas.

A política deve informar motivo, escopo, prazo, exceções e consequências
de não conformidade.

### 9.3 Inventário e conformidade

-   Hardware e software instalado.
-   Versões de sistema e aplicativos.
-   Atualizações pendentes.
-   Criptografia e controles de segurança, quando detectáveis.
-   Estado de políticas.
-   Dispositivos sem contato recente.
-   Aplicativos não aprovados, conforme política e capacidade técnica.
-   Histórico de alterações.

Não afirmar conformidade com padrões regulatórios sem implementar e
validar os controles necessários. Relatórios devem deixar claro quais
dados foram realmente verificados.

### 9.4 Onboarding e offboarding de funcionários

Possíveis fluxos: - Preparar um dispositivo com preset de função. -
Associar dispositivo ao funcionário ou grupo. - Aplicar aplicativos e
configurações. - Revogar sessões e acesso quando houver desligamento. -
Remover ou transferir dispositivo. - Apagar dados corporativos apenas
quando houver mecanismo apropriado e autorização.

A plataforma não deve apagar dados pessoais de dispositivos mistos sem
consentimento, política clara e separação técnica entre dados pessoais e
corporativos.

### 9.5 Atualizações empresariais

-   Anéis de atualização: laboratório, piloto, produção.
-   Janelas de manutenção.
-   Aprovação de atualizações críticas.
-   Adiamento controlado.
-   Reinicializações planejadas.
-   Relatório de sucesso e falhas.
-   Política de drivers e firmware.
-   Possibilidade de testar atualizações em um grupo pequeno antes da
    frota.

### 9.6 Auditoria e governança

Registrar: - Quem solicitou uma ação. - Quem aprovou. - Qual dispositivo
foi afetado. - O que foi alterado. - Resultado. - Horário. - Versão da
política. - Origem da tarefa (usuário, API, automação ou integração).

Permitir exportação de auditoria e retenção configurável.

### 9.7 Identidade empresarial

Além do TruthID opcional, avaliar no futuro: - OpenID Connect. - SAML. -
SCIM para provisionamento de usuários e grupos. - MFA e políticas de
sessão. - Integração com provedores corporativos.

Não presumir que TruthID substituirá todos os provedores de identidade
empresariais.

### 9.8 Modelo comercial empresarial

Possibilidades futuras: - Open-source self-hosted. - Plano gerenciado em
cloud. - Licença ou assinatura por dispositivo, usuário ou capacidade. -
Recursos empresariais: SSO, RBAC avançado, auditoria, políticas,
relatórios, suporte e gestão em escala. - Suporte profissional e
implantação. - Integrações e plugins especializados.

A estratégia de licenciamento e quais recursos serão pagos devem ser
decididas depois de validar o produto. Não criar artificialmente
limitações que prejudiquem a versão open-source.

------------------------------------------------------------------------

## 10. Segurança e privacidade

O Universal terá acesso a informações sensíveis e, em alguns casos,
poderá alterar o sistema operacional. Segurança precisa ser parte da
arquitetura inicial.

### 10.1 Princípios

-   Privilégio mínimo.
-   Separação entre leitura e escrita.
-   Elevação administrativa somente quando necessária.
-   Nenhum shell remoto irrestrito por padrão.
-   Ações com escopo, parâmetros e validação.
-   Aprovação para operações sensíveis.
-   Auditoria.
-   Revogação de dispositivos e tokens.
-   Criptografia em trânsito e em repouso.
-   Atualizações assinadas dos agentes e componentes.
-   Proteção contra downgrade e execução de binários não confiáveis.

### 10.2 Segredos

Nunca sincronizar como arquivos comuns: - Senhas. - Tokens de API. -
Chaves privadas SSH. - Credenciais de cloud. - Segredos de produção. -
Arquivos `.env` com credenciais.

Criar integração específica com secret managers ou armazenamento
criptografado, com acesso por dispositivo e escopo. O preset pode
referenciar um segredo, mas não deve conter seu valor.

### 10.3 Ações sensíveis

Exigir aprovação explícita ou política administrativa controlada para: -
Instalar drivers e firmware. - Alterar firewall. - Alterar rede. -
Overclock e undervolt. - Desativar serviços. - Reiniciar ou desligar
máquinas. - Remover aplicativos e dados. - Executar testes de stress. -
Alterar configurações de segurança. - Executar comandos administrativos.

### 10.4 Privacidade

-   Coletar somente o necessário.
-   Permitir desativar categorias de telemetria.
-   Exibir o que é coletado.
-   Definir retenção.
-   Exportar e apagar dados conforme modelo de propriedade e obrigações
    aplicáveis.
-   Separar dados pessoais e corporativos quando possível.
-   Não transformar o produto em ferramenta de vigilância oculta de
    funcionários.

Monitoramento empresarial deve ser transparente, proporcional,
autorizado e adequado à legislação aplicável.

------------------------------------------------------------------------

## 11. Plugins, extensibilidade e ecossistema

Criar contratos estáveis para adaptadores.

Tipos de plugin: - Hardware e sensores. - Sistemas operacionais. -
Gerenciadores de pacotes. - Periféricos. - Provedores de VPS/cloud. -
Hypervisors. - Monitoramento e observabilidade. - Git e plataformas de
desenvolvimento. - Identidade e autenticação. - Notificações. -
Ferramentas empresariais.

Cada plugin deve declarar: - Identificador e versão. -
Compatibilidade. - Permissões necessárias. - Dados coletados. - Ações
oferecidas. - Dependências. - Licença. - Procedimento de atualização e
remoção.

Plugins de terceiros devem ser tratados como código não confiável até
validação. Considerar isolamento, assinatura, revisão e permissões
limitadas.

------------------------------------------------------------------------

## 12. Dados e armazenamento

### PostgreSQL

Dados relacionais: - Usuários e organizações. - Papéis e permissões. -
Dispositivos e agentes. - Presets e versões. - Políticas. - Projetos,
serviços e ambientes. - Integrações. - Tarefas e execuções. - Alertas e
auditoria.

### Métricas e eventos

Não armazenar séries temporais de alta frequência indefinidamente em
tabelas relacionais comuns sem estratégia.

Avaliar: - Prometheus para métricas e consultas. - TimescaleDB para
séries temporais integradas ao PostgreSQL. - Armazenamento de logs
separado. - Retenção e downsampling. - Buffer local no agente.

### Versionamento

Presets, políticas e configurações devem ter versões identificáveis.
Permitir comparar versões, visualizar diferenças e reverter para uma
versão anterior.

------------------------------------------------------------------------

## 13. UX e navegação sugeridas

Navegação principal:

1.  Overview
2.  Devices
3.  Environments
4.  Applications
5.  Workspace
6.  Updates
7.  Infrastructure
8.  Projects
9.  Automations
10. Integrations
11. Organization Settings

Em cada dispositivo: - Overview. - Hardware. - Performance. -
Applications. - Updates. - Environment. - Peripherals. - Processes &
Services. - Events. - Settings.

Em cada projeto: - Overview. - Uptime. - Requests. - Latency. -
Errors. - Logs. - Traces. - Deployments. - Dependencies. -
Infrastructure.

A interface deve priorizar clareza e densidade informacional sem parecer
um painel de operações complexo para usuários comuns. Recursos
empresariais avançados podem ficar em áreas administrativas.

------------------------------------------------------------------------

## 14. Roadmap proposto

O roadmap deve ser validado com protótipos. Não tentar construir todas
as áreas ao mesmo tempo.

### Fase 0 --- Pesquisa e provas técnicas

-   Investigar APIs e bibliotecas de hardware.
-   Testar coleta em Arch Linux e Windows.
-   Avaliar Tauri e Rust.
-   Testar instalação de pacotes por adaptadores.
-   Prototipar Configuration Engine.
-   Avaliar OpenTelemetry e Prometheus.
-   Estudar licenças de projetos que possam ser reutilizados.
-   Definir modelo de ameaças e permissões.

### Fase 1 --- MVP local no Arch Linux

Prioridade: resolver primeiro o problema real de padronizar o ambiente
de desenvolvimento.

-   Aplicativo desktop.
-   Agente local.
-   Inventário básico.
-   Presets declarativos.
-   Aplicativos obrigatórios/opcionais/exclusivos.
-   Instalação via pacman e Flatpak.
-   Aparência e dotfiles suportados.
-   Detecção de pasta de workspace.
-   Cadastro de repositórios Git.
-   Fetch e diagnóstico seguro de sincronização.
-   Central de atualizações básica.
-   Histórico e reversão de configurações suportadas.

### Fase 2 --- Windows e multi-device

-   Adaptadores Windows.
-   WinGet e Windows Update.
-   Core central.
-   Cadastro de múltiplos dispositivos.
-   Sincronização de presets.
-   Grupos e exceções por dispositivo.
-   Tarefas remotas com aprovação.
-   Dashboard multi-device.

### Fase 3 --- Linux ampliado e infraestrutura

-   Debian/Ubuntu e Fedora.
-   DNF, APT e outros adaptadores priorizados.
-   Agente de servidor.
-   VPS e homelab.
-   Docker.
-   Proxmox.
-   Métricas e alertas.
-   Monitoramento de serviços.

### Fase 4 --- Project Health

-   Health checks.
-   Uptime.
-   Requisições e latência.
-   OpenTelemetry.
-   Logs, traces e erros.
-   Deploys e correlação com infraestrutura.
-   Dashboards por projeto e ambiente.

### Fase 5 --- Empresa

-   Multi-tenancy robusto.
-   RBAC avançado.
-   Grupos e políticas.
-   Enrollment em escala.
-   SSO/OIDC e SAML.
-   SCIM.
-   Auditoria e relatórios.
-   Anéis de atualização.
-   Compliance e governança.
-   APIs administrativas.
-   Testes de isolamento entre organizações.

### Fase 6 --- Ecossistema avançado

-   macOS.
-   Periféricos e RGB.
-   Benchmarks avançados.
-   Otimização por jogo.
-   Plugins comunitários.
-   Cloud gerenciada.
-   Integração opcional com TruthID e agente de IA.
-   Automação avançada e recomendações.

------------------------------------------------------------------------

## 15. O que não fazer no início

-   Não começar com microserviços.
-   Não tentar suportar todas as distribuições Linux de uma vez.
-   Não prometer controle de todo periférico.
-   Não implementar overclock e undervolt no MVP.
-   Não executar scripts de otimização genéricos.
-   Não fazer pull/reset de Git sem inspeção.
-   Não sincronizar segredos em presets.
-   Não tornar a IA obrigatória.
-   Não depender de um único provedor de identidade.
-   Não criar um sistema próprio de métricas quando padrões existentes
    resolvem.
-   Não construir uma solução empresarial completa antes de validar o
    uso pessoal e multi-device.
-   Não misturar gerenciamento de endpoints com monitoramento de
    aplicações sem limites claros entre módulos.

------------------------------------------------------------------------

## 16. Critérios de sucesso do MVP

O MVP inicial será considerado útil quando for possível:

1.  Instalar o Universal em um Arch Linux.
2.  Identificar hardware, sistema e aplicativos.
3.  Criar um preset com aplicativos e configurações.
4.  Reaplicar o preset sem duplicar ou quebrar configurações.
5.  Detectar aplicativo recém-instalado e perguntar se deve entrar no
    preset.
6.  Identificar a pasta padrão de workspace.
7.  Descobrir repositórios Git e exibir seu estado.
8.  Verificar atualizações de pacotes e aplicativos.
9.  Exibir um plano antes de executar mudanças.
10. Registrar o que foi alterado e permitir reversão quando suportada.

Critérios para a fase multi-device:

1.  Registrar dois computadores na mesma organização.
2.  Aplicar o mesmo preset nos dois, respeitando capacidades diferentes.
3.  Sincronizar mudanças do preset.
4.  Exibir divergências e falhas.
5.  Manter operação local quando o Core estiver indisponível.

------------------------------------------------------------------------

## 17. Instruções para iniciar o trabalho com Claude Code

Antes de escrever grandes quantidades de código, Claude deve atuar como
arquiteto e pesquisador técnico.

### Primeira tarefa

1.  Ler este documento inteiro.
2.  Identificar requisitos ambíguos, riscos e decisões arquiteturais
    ainda abertas.
3.  Pesquisar bibliotecas e projetos existentes que possam ser
    reutilizados, verificando licença, manutenção, compatibilidade e
    limitações.
4.  Propor uma arquitetura inicial simples, modular e testável.
5.  Separar requisitos do MVP de ideias futuras.
6.  Criar um documento de decisões arquiteturais (ADRs).
7.  Propor estrutura de monorepo ou repositórios separados, justificando
    a decisão.
8.  Definir interfaces entre Core, Agent, Desktop e adaptadores.
9.  Criar um plano de testes, incluindo testes de segurança e de
    idempotência.
10. Não implementar tudo de uma vez.

### Preferência de desenvolvimento

-   Explicar decisões arquiteturais em linguagem clara.
-   Priorizar compreensão e manutenção, não apenas velocidade de geração
    de código.
-   Evitar abstrações prematuras.
-   Criar testes desde os primeiros módulos.
-   Não esconder operações perigosas atrás de botões genéricos.
-   Documentar limitações por sistema operacional.
-   Utilizar bibliotecas maduras antes de implementar protocolos
    próprios.
-   Manter o produto utilizável em modo local.
-   Não assumir que um recurso é multiplataforma sem verificar.
-   Para cada funcionalidade, definir: capacidades, permissões, riscos,
    estado atual, estado desejado, plano, execução, resultado e
    reversão.

### Primeira entrega esperada do Claude

Produzir: - `ARCHITECTURE.md` - `MVP.md` - `ROADMAP.md` -
`SECURITY.md` - `ADRs/` - Esqueleto do projeto, somente depois de
validar a arquitetura.

O primeiro protótipo deve resolver o caso real: padronizar dois
ambientes Arch Linux, gerenciar aplicativos, dotfiles e workspace Git
com segurança.

------------------------------------------------------------------------

## 18. Decisões em aberto

Não tratar as hipóteses deste documento como decisões irreversíveis.
Validar:

-   Nome e identidade visual do produto.
-   Monorepo versus múltiplos repositórios.
-   Framework e arquitetura final do Core.
-   Estratégia de armazenamento de métricas.
-   Protocolo de comunicação com agentes.
-   Estratégia de enrollment e provisionamento empresarial.
-   Modelo de autenticação: local, TruthID, OIDC e outros.
-   Licença open-source.
-   Modelo de monetização.
-   Retenção e privacidade de telemetria.
-   Quais módulos devem funcionar sem Core.
-   Estratégia de plugins e isolamento.
-   Suporte prioritário a cada sistema e gerenciador de pacotes.

------------------------------------------------------------------------

## 19. Definição final

**Universal é uma plataforma aberta e modular para administrar o
ambiente computacional de pessoas e organizações.**

Ela conecta computadores, servidores, aplicações e projetos; padroniza
ambientes; sincroniza configurações; gerencia aplicativos e
atualizações; monitora saúde e desempenho; e oferece automações seguras.

O objetivo é permitir que um ambiente computacional deixe de depender de
uma máquina específica e passe a ser algo reproduzível, administrável e
portátil.

Para uma pessoa, isso significa instalar um computador novo e recuperar
seu ambiente de trabalho.

Para uma equipe, significa padronizar ferramentas e acompanhar seus
projetos.

Para uma empresa, significa administrar endpoints, servidores e
aplicações com políticas, permissões, auditoria e governança.

A IA, TruthID e outros projetos podem se integrar ao Universal, mas
nenhum deles é obrigatório para o funcionamento da plataforma.
