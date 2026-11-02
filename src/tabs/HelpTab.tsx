import { useState } from "react";
import { ChevronDown } from "lucide-react";
import { useT } from "../i18n";
import { PageHeader } from "../components/ui";
import { tabIcon } from "../icons";

type Verdict = "real" | "pouco" | "placebo";

const VERDICTS: { name: string; verdict: Verdict; why: string }[] = [
  { name: "Plano de energia de alto desempenho", verdict: "real", why: "Impede a CPU de reduzir a frequência. Maior efeito em notebooks e PCs com economia agressiva." },
  { name: "Aceleração do mouse desligada", verdict: "real", why: "Movimento 1:1, mira mais consistente. Não muda o FPS." },
  { name: "Fechar programas em segundo plano", verdict: "real", why: "Libera CPU, RAM e disco. É o ganho mais honesto em PCs com pouca memória." },
  { name: "Remover aplicativos pré-instalados", verdict: "pouco", why: "Libera disco e alguns MB de RAM. Não aumenta FPS." },
  { name: "Desativar Game DVR e Game Bar", verdict: "pouco", why: "Pode reduzir overhead em GPUs fracas; o ganho costuma ser pequeno." },
  { name: "Desativar serviços do Windows", verdict: "pouco", why: "Libera pouca RAM e traz risco de quebrar atualizações. Faça só os listados como seguros." },
  { name: "HAGS, timer e MSI Mode", verdict: "pouco", why: "Dependem do jogo e do driver: ajudam, não mudam ou pioram. Use o teste A/B." },
  { name: "Desativar a Integridade de Memória (HVCI)", verdict: "pouco", why: "Pode dar alguns % em certos PCs, mas reduz a proteção contra drivers maliciosos." },
  { name: "Ajustes de TCP e Nagle", verdict: "placebo", why: "Jogos usam UDP; esses ajustes não reduzem o ping. O ping depende da rota até o servidor." },
  { name: "\"Core parking\" e prioridade de CPU por registro", verdict: "placebo", why: "Sem efeito mensurável em CPUs modernas. O Nexus não oferece esses ajustes." },
  { name: "Apagar o cache de shaders", verdict: "placebo", why: "Piora: causa travadas até o cache ser recriado. O Nexus não apaga esse cache." },
  { name: "Liberar RAM", verdict: "placebo", why: "Efeito temporário: o Windows devolve as páginas quando o programa precisa delas." },
];

const FAQ: { q: string; a: string }[] = [
  { q: "Os ajustes são seguros?", a: "Cada ajuste mostra o risco (seguro, moderado ou avançado). Antes de mexer no registro, o Nexus guarda o valor original e o botão Reverter o restaura. Para os avançados ele cria um ponto de restauração antes, se você deixar essa opção ligada." },
  { q: "O Nexus envia dados do meu PC?", a: "Não há telemetria. A internet é usada só para o teste de velocidade, o catálogo de jogos (para reconhecer o processo e o ícone) e a verificação de atualizações. Nada do seu PC é enviado." },
  { q: "Por que o benchmark pede administrador?", a: "Ele usa o PresentMon, que lê o desempenho da placa de vídeo pelo rastreamento de eventos do Windows. Isso só funciona com administrador." },
  { q: "O que é o Modo Jogo automático?", a: "Quando um jogo do catálogo abre, o Nexus aplica o perfil dele (prioridade, ajustes, programas a fechar). Quando o jogo fecha, desfaz o que ele mesmo mudou. Precisa que o app esteja aberto ou na bandeja." },
  { q: "Por que meu jogo não aparece na lista do benchmark?", a: "O catálogo reconhece cerca de 10 mil jogos pelo nome do processo. Se o seu não estiver lá, use \"Meu jogo não aparece\" e digite o nome do executável (Gerenciador de Tarefas, aba Detalhes)." },
  { q: "Como desfaço tudo?", a: "Em Início, use \"Reverter tudo\". Você também pode criar um ponto de restauração do Windows em Sistema e consultar o histórico de atividades em Configurações." },
  { q: "O ping do teste de velocidade é diferente do ping do meu jogo?", a: "Sim. O teste mede a conexão até um servidor de internet; o jogo mede até o servidor dele. Use Rede para medir regiões e serviços de jogos e para ver a rota." },
  { q: "O Nexus faz overlay de FPS dentro do jogo?", a: "Não, de propósito: overlays podem ser vistos como adulteração por sistemas anticheat. Para medir FPS, use o Benchmark." },
];

const BADGE: Record<Verdict, string> = { real: "Efeito real", pouco: "Efeito pequeno", placebo: "Placebo" };

export function HelpTab() {
  const t = useT();
  const [open, setOpen] = useState<number | null>(0);
  return (
    <>
      <PageHeader icon={tabIcon("Ajuda")} title={t("Ajuda")} desc={t("O que cada coisa faz de verdade, sem exagero.")} />

      <section className="card">
        <h3>{t("Real, pequeno ou placebo")}</h3>
        <p className="muted">{t("Muitos otimizadores prometem FPS infinito. Aqui está o que costuma funcionar e o que não funciona. Meça sempre com o Benchmark.")}</p>
        <ul className="verdicts">
          {VERDICTS.map((v) => (
            <li key={v.name}>
              <span className={`badge b-${v.verdict}`}>{t(BADGE[v.verdict])}</span>
              <div>
                <b>{t(v.name)}</b>
                <small>{t(v.why)}</small>
              </div>
            </li>
          ))}
        </ul>
      </section>

      <section className="card">
        <h3>{t("Perguntas frequentes")}</h3>
        <ul className="faq">
          {FAQ.map((f, i) => (
            <li key={f.q} className={open === i ? "open" : ""}>
              <button className="faq-q" onClick={() => setOpen(open === i ? null : i)}>
                <span>{t(f.q)}</span>
                <ChevronDown size={16} />
              </button>
              {open === i && <p>{t(f.a)}</p>}
            </li>
          ))}
        </ul>
      </section>
    </>
  );
}
