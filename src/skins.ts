/**
 * 皮肤系统：整幅背景场景（矢量手绘 SVG），替换主界面的默认氛围背景。
 *
 * - 每套皮肤是一份 1600×1000 的 SVG 场景（多层渐变 + 剪影 + 氛围元素），
 *   以 data URI 作为背景图，background-size: cover 随窗口裁切；
 * - 皮肤之上仍叠加：主题化压暗/提亮纱（--skin-scrim，保证两种主题下
 *   文字可读）、封面取色微光、暗角与噪点——玻璃面板透出的是"加了音乐
 *   氛围光的壁纸"而非一张死图；
 * - 与浅色/暗色主题、强调色自由组合，localStorage 持久化。
 */

export interface Skin {
  key: string;
  name: string;
  desc: string;
  /** 完整 SVG 源码（单引号属性，无文字，纯矢量场景） */
  svg: string;
  /** 深色主题的纱：压暗背景保证浅色文字可读（按场景明暗单独调） */
  scrimDark: string;
  /** 浅色主题的纱：提亮背景保证深色文字可读（按场景明暗单独调） */
  scrimLight: string;
}

const SVG_HEAD =
  "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1600 1000' preserveAspectRatio='xMidYMid slice'>";

/* ---------- 星夜：深蓝夜空、银河星子、山月剪影 ---------- */
const STARRY = `${SVG_HEAD}
<defs>
<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#080d28'/><stop offset='.5' stop-color='#161f47'/><stop offset='1' stop-color='#2c3a6e'/>
</linearGradient>
<radialGradient id='moon' cx='.5' cy='.5' r='.5'>
<stop offset='0' stop-color='#fdf7da' stop-opacity='.95'/><stop offset='.42' stop-color='#f7ecb9' stop-opacity='.5'/><stop offset='1' stop-color='#f7ecb9' stop-opacity='0'/>
</radialGradient>
<linearGradient id='milky' x1='0' y1='0' x2='1' y2='1'>
<stop offset='0' stop-color='#9fb4ff' stop-opacity='0'/><stop offset='.5' stop-color='#b9c6ff' stop-opacity='.14'/><stop offset='1' stop-color='#9fb4ff' stop-opacity='0'/>
</linearGradient>
</defs>
<rect width='1600' height='1000' fill='url(#sky)'/>
<rect width='1600' height='1000' fill='url(#milky)' transform='rotate(-24 800 400)'/>
<g fill='#ffffff'>
<circle cx='120' cy='140' r='1.6' opacity='.85'/><circle cx='233' cy='88' r='1.1' opacity='.5'/><circle cx='318' cy='212' r='1.4' opacity='.7'/><circle cx='402' cy='96' r='1' opacity='.45'/><circle cx='476' cy='286' r='1.7' opacity='.8'/><circle cx='560' cy='150' r='1.1' opacity='.5'/><circle cx='648' cy='72' r='1.3' opacity='.65'/><circle cx='722' cy='232' r='1' opacity='.4'/><circle cx='836' cy='120' r='1.6' opacity='.8'/><circle cx='928' cy='258' r='1.1' opacity='.5'/><circle cx='1046' cy='86' r='1.3' opacity='.6'/><circle cx='1132' cy='190' r='1' opacity='.42'/><circle cx='1258' cy='330' r='1.4' opacity='.7'/><circle cx='1372' cy='128' r='1.2' opacity='.55'/><circle cx='1478' cy='248' r='1.6' opacity='.75'/><circle cx='1544' cy='96' r='1' opacity='.45'/><circle cx='196' cy='330' r='1.2' opacity='.55'/><circle cx='88' cy='420' r='1' opacity='.4'/><circle cx='502' cy='404' r='1.3' opacity='.6'/><circle cx='700' cy='368' r='1' opacity='.45'/><circle cx='912' cy='420' r='1.4' opacity='.65'/><circle cx='1090' cy='378' r='1' opacity='.4'/><circle cx='1330' cy='440' r='1.2' opacity='.5'/><circle cx='1520' cy='392' r='1.3' opacity='.6'/>
</g>
<circle cx='1210' cy='205' r='150' fill='url(#moon)'/>
<circle cx='1210' cy='205' r='58' fill='#fbf3cf'/>
<circle cx='1188' cy='188' r='10' fill='#eddfae' opacity='.7'/><circle cx='1226' cy='216' r='7' fill='#eddfae' opacity='.6'/><circle cx='1206' cy='232' r='5' fill='#eddfae' opacity='.5'/>
<path d='M0 660 L150 570 300 638 470 552 630 648 800 578 970 660 1150 588 1330 668 1480 606 1600 656 V1000 H0 Z' fill='#151d3f' opacity='.95'/>
<path d='M0 742 L190 668 360 730 540 660 720 742 900 676 1090 750 1270 686 1440 754 1600 700 V1000 H0 Z' fill='#0d1330'/>
<path d='M0 846 L220 782 420 836 640 776 860 846 1080 790 1300 852 1600 796 V1000 H0 Z' fill='#070b20'/>
<ellipse cx='420' cy='700' rx='480' ry='34' fill='#9fb0ff' opacity='.06'/>
<ellipse cx='1150' cy='790' rx='520' ry='38' fill='#9fb0ff' opacity='.05'/>
</svg>`;

/* ---------- 极光：寒夜雪原上流动的光幕 ---------- */
const AURORA = `${SVG_HEAD}
<defs>
<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#040b18'/><stop offset='.55' stop-color='#082031'/><stop offset='1' stop-color='#0d3542'/>
</linearGradient>
<linearGradient id='g1' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#5df2a2' stop-opacity='0'/><stop offset='.5' stop-color='#5df2a2' stop-opacity='.65'/><stop offset='1' stop-color='#2ec4a6' stop-opacity='0'/>
</linearGradient>
<linearGradient id='g2' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#7ce8e0' stop-opacity='0'/><stop offset='.5' stop-color='#7ce8e0' stop-opacity='.5'/><stop offset='1' stop-color='#3f9fd8' stop-opacity='0'/>
</linearGradient>
<linearGradient id='g3' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#9c86f2' stop-opacity='0'/><stop offset='.5' stop-color='#9c86f2' stop-opacity='.4'/><stop offset='1' stop-color='#9c86f2' stop-opacity='0'/>
</linearGradient>
<filter id='soft' x='-20%' y='-40%' width='140%' height='180%'><feGaussianBlur stdDeviation='20'/></filter>
</defs>
<rect width='1600' height='1000' fill='url(#sky)'/>
<g fill='#dff5ff'>
<circle cx='150' cy='120' r='1.4' opacity='.7'/><circle cx='320' cy='80' r='1' opacity='.45'/><circle cx='486' cy='168' r='1.2' opacity='.6'/><circle cx='640' cy='70' r='1.1' opacity='.5'/><circle cx='810' cy='150' r='1.4' opacity='.7'/><circle cx='980' cy='96' r='1' opacity='.45'/><circle cx='1150' cy='180' r='1.2' opacity='.6'/><circle cx='1330' cy='84' r='1.4' opacity='.7'/><circle cx='1490' cy='160' r='1' opacity='.5'/><circle cx='240' cy='260' r='1' opacity='.4'/><circle cx='560' cy='300' r='1.1' opacity='.45'/><circle cx='900' cy='330' r='1.2' opacity='.5'/><circle cx='1240' cy='280' r='1' opacity='.4'/><circle cx='1440' cy='340' r='1.1' opacity='.45'/>
</g>
<path d='M-80 430 C 240 190, 480 320, 780 180 C 1040 60, 1280 210, 1680 120 V 330 C 1300 380, 1060 240, 800 340 C 520 440, 260 320, -80 560 Z' fill='url(#g1)' filter='url(#soft)'/>
<path d='M-80 560 C 300 340, 620 460, 920 320 C 1160 210, 1400 330, 1680 260 V 430 C 1380 480, 1140 360, 880 460 C 600 560, 280 460, -80 680 Z' fill='url(#g2)' filter='url(#soft)' opacity='.8'/>
<path d='M-80 300 C 280 130, 560 240, 860 120 C 1120 20, 1380 140, 1680 60 V 200 C 1360 260, 1100 150, 820 260 C 540 360, 240 240, -80 420 Z' fill='url(#g3)' filter='url(#soft)' opacity='.65'/>
<path d='M0 760 L130 700 260 745 420 672 580 742 740 680 900 750 1080 690 1260 756 1440 700 1600 748 V1000 H0 Z' fill='#0a1a28' opacity='.95'/>
<path d='M0 862 L210 800 400 850 620 790 840 856 1060 802 1280 858 1600 806 V1000 H0 Z' fill='#050f1a'/>
<ellipse cx='800' cy='900' rx='720' ry='60' fill='#39c9b0' opacity='.05'/>
</svg>`;

/* ---------- 晚霞：海面落日、云霞与归鸟 ---------- */
const SUNSET = `${SVG_HEAD}
<defs>
<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#33194e'/><stop offset='.34' stop-color='#7c2f63'/><stop offset='.58' stop-color='#d15c74'/><stop offset='.8' stop-color='#ff9d6a'/><stop offset='1' stop-color='#ffd8a4'/>
</linearGradient>
<linearGradient id='sea' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#83325f'/><stop offset='.45' stop-color='#3a1748'/><stop offset='1' stop-color='#1c0c32'/>
</linearGradient>
<radialGradient id='sun' cx='.5' cy='.5' r='.5'>
<stop offset='0' stop-color='#fff6dd' stop-opacity='.95'/><stop offset='.4' stop-color='#ffdf9e' stop-opacity='.5'/><stop offset='1' stop-color='#ffdf9e' stop-opacity='0'/>
</radialGradient>
<linearGradient id='cl' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#ffb08c' stop-opacity='.85'/><stop offset='1' stop-color='#e2688a' stop-opacity='.35'/>
</linearGradient>
</defs>
<rect width='1600' height='620' fill='url(#sky)'/>
<circle cx='820' cy='548' r='200' fill='url(#sun)'/>
<circle cx='820' cy='548' r='74' fill='#fff3d2'/>
<g fill='url(#cl)'>
<ellipse cx='330' cy='250' rx='240' ry='22' opacity='.7'/><ellipse cx='520' cy='300' rx='170' ry='14' opacity='.55'/><ellipse cx='1180' cy='210' rx='290' ry='24' opacity='.75'/><ellipse cx='1010' cy='300' rx='190' ry='15' opacity='.6'/><ellipse cx='1400' cy='330' rx='210' ry='16' opacity='.5'/><ellipse cx='240' cy='380' rx='190' ry='13' opacity='.45'/>
</g>
<g stroke='#33203f' stroke-width='5' fill='none' stroke-linecap='round' opacity='.8'>
<path d='M600 180 q 14 -14 28 0'/><path d='M646 158 q 12 -12 24 0'/><path d='M1262 140 q 13 -13 26 0'/>
</g>
<rect y='620' width='1600' height='380' fill='url(#sea)'/>
<g fill='#ffc894'>
<ellipse cx='820' cy='648' rx='150' ry='7' opacity='.5'/><ellipse cx='820' cy='676' rx='110' ry='6' opacity='.4'/><ellipse cx='820' cy='712' rx='160' ry='7' opacity='.28'/><ellipse cx='820' cy='756' rx='90' ry='5' opacity='.24'/><ellipse cx='820' cy='812' rx='140' ry='7' opacity='.16'/><ellipse cx='820' cy='880' rx='70' ry='5' opacity='.12'/>
</g>
<path d='M0 912 Q 400 884 800 910 T 1600 902 V1000 H0 Z' fill='#170a2b' opacity='.9'/>
<path d='M0 956 Q 420 934 860 954 T 1600 948 V1000 H0 Z' fill='#10061f'/>
<path d='M60 700 q 120 26 240 6' stroke='#ff9d8a' stroke-width='3' fill='none' opacity='.3'/>
<path d='M1180 730 q 130 22 250 2' stroke='#ff9d8a' stroke-width='3' fill='none' opacity='.25'/>
</svg>`;

/* ---------- 水墨：远山淡影、留白与朱砂 ---------- */
const INK = `${SVG_HEAD}
<defs>
<linearGradient id='paper' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#f4efe2'/><stop offset='1' stop-color='#e6dec9'/>
</linearGradient>
<filter id='wash' x='-20%' y='-20%' width='140%' height='140%'><feGaussianBlur stdDeviation='7'/></filter>
<filter id='wash2' x='-20%' y='-20%' width='140%' height='140%'><feGaussianBlur stdDeviation='14'/></filter>
</defs>
<rect width='1600' height='1000' fill='url(#paper)'/>
<circle cx='1300' cy='185' r='46' fill='#bf4034' opacity='.78'/>
<g filter='url(#wash2)'>
<path d='M-40 470 Q 210 300 430 440 Q 600 340 820 430 Q 1010 330 1220 430 Q 1400 350 1640 440 V 620 H -40 Z' fill='#8b8d99' opacity='.2'/>
</g>
<g filter='url(#wash)'>
<path d='M-40 560 Q 170 420 380 530 Q 560 440 760 540 Q 950 450 1160 550 Q 1360 470 1640 560 V 720 H -40 Z' fill='#6f7280' opacity='.26'/>
</g>
<ellipse cx='420' cy='600' rx='500' ry='46' fill='#f6f1e4' opacity='.85' filter='url(#wash2)'/>
<ellipse cx='1240' cy='640' rx='480' ry='42' fill='#f6f1e4' opacity='.8' filter='url(#wash2)'/>
<path d='M-40 690 Q 240 560 480 670 Q 700 580 920 690 Q 1150 600 1380 700 Q 1520 650 1640 690 V 900 H -40 Z' fill='#4c4e5c' opacity='.34' filter='url(#wash)'/>
<ellipse cx='820' cy='760' rx='620' ry='52' fill='#f4efe2' opacity='.7' filter='url(#wash2)'/>
<path d='M-40 880 Q 300 800 640 866 Q 980 812 1300 876 Q 1480 842 1640 868 V 1000 H -40 Z' fill='#3a3c49' opacity='.5'/>
<g stroke='#3f414d' stroke-width='7' fill='none' stroke-linecap='round' opacity='.8'>
<path d='M60 120 Q 300 220 560 190'/><path d='M300 172 Q 360 130 428 140'/><path d='M430 186 Q 500 160 540 172'/>
</g>
<g stroke='#3f414d' stroke-width='4.5' fill='none' stroke-linecap='round' opacity='.75'>
<path d='M760 240 q 16 -18 32 0'/><path d='M812 216 q 14 -16 28 0'/><path d='M868 244 q 15 -17 30 0'/>
</g>
</svg>`;

/* ---------- 霓虹：合成波落日、网格地平与都市灯影 ---------- */
const NEON = `${SVG_HEAD}
<defs>
<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#0c0322'/><stop offset='.5' stop-color='#25073e'/><stop offset='1' stop-color='#4c1168'/>
</linearGradient>
<linearGradient id='sun' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#ffe29a'/><stop offset='.5' stop-color='#ff5ea8'/><stop offset='1' stop-color='#a83ce0'/>
</linearGradient>
<filter id='glow' x='-60%' y='-60%' width='220%' height='220%'><feGaussianBlur stdDeviation='10'/></filter>
</defs>
<rect width='1600' height='1000' fill='url(#sky)'/>
<g fill='#ffd9f2'>
<circle cx='180' cy='130' r='1.5' opacity='.8'/><circle cx='360' cy='90' r='1.1' opacity='.5'/><circle cx='540' cy='180' r='1.3' opacity='.65'/><circle cx='1010' cy='110' r='1.4' opacity='.7'/><circle cx='1190' cy='200' r='1.1' opacity='.5'/><circle cx='1400' cy='120' r='1.5' opacity='.8'/><circle cx='1520' cy='240' r='1.1' opacity='.5'/><circle cx='300' cy='300' r='1' opacity='.4'/><circle cx='1300' cy='330' r='1.2' opacity='.5'/>
</g>
<circle cx='800' cy='430' r='185' fill='url(#sun)' filter='url(#glow)' opacity='.65'/>
<circle cx='800' cy='430' r='160' fill='url(#sun)'/>
<g fill='#25073e'>
<rect x='560' y='392' width='480' height='12' opacity='.85'/><rect x='560' y='428' width='480' height='16' opacity='.85'/><rect x='560' y='470' width='480' height='20' opacity='.85'/><rect x='560' y='518' width='480' height='24' opacity='.85'/><rect x='560' y='572' width='480' height='28' opacity='.85'/>
</g>
<g fill='#0b0219'>
<rect x='0' y='470' width='120' height='180'/><rect x='130' y='510' width='90' height='140'/><rect x='235' y='440' width='130' height='210'/><rect x='380' y='530' width='100' height='120'/><rect x='495' y='480' width='110' height='170'/><rect x='1470' y='450' width='130' height='200'/><rect x='1350' y='520' width='100' height='130'/><rect x='1180' y='470' width='150' height='180'/><rect x='1040' y='540' width='120' height='110'/>
</g>
<g fill='#ff9ade' opacity='.65'>
<rect x='20' y='492' width='8' height='6'/><rect x='60' y='520' width='8' height='6'/><rect x='105' y='500' width='8' height='6'/><rect x='255' y='470' width='8' height='6'/><rect x='300' y='500' width='8' height='6'/><rect x='345' y='462' width='8' height='6'/><rect x='515' y='502' width='8' height='6'/><rect x='1200' y='492' width='8' height='6'/><rect x='1250' y='522' width='8' height='6'/><rect x='1380' y='545' width='8' height='6'/><rect x='1500' y='480' width='8' height='6'/>
</g>
<rect y='640' width='1600' height='340' fill='#12042a'/>
<rect x='0' y='637' width='1600' height='6' fill='#ff7ad9' opacity='.9' filter='url(#glow)'/>
<g stroke='#ff5ec4' stroke-width='2' opacity='.55'>
<line x1='800' y1='643' x2='-320' y2='1000'/><line x1='800' y1='643' x2='40' y2='1000'/><line x1='800' y1='643' x2='400' y2='1000'/><line x1='800' y1='643' x2='800' y2='1000'/><line x1='800' y1='643' x2='1200' y2='1000'/><line x1='800' y1='643' x2='1560' y2='1000'/><line x1='800' y1='643' x2='1920' y2='1000'/>
</g>
<g stroke='#8a3ff0' stroke-width='2' opacity='.6'>
<line x1='0' y1='668' x2='1600' y2='668'/><line x1='0' y1='702' x2='1600' y2='702'/><line x1='0' y1='746' x2='1600' y2='746'/><line x1='0' y1='802' x2='1600' y2='802'/><line x1='0' y1='874' x2='1600' y2='874'/><line x1='0' y1='962' x2='1600' y2='962'/>
</g>
</svg>`;

/* ---------- 晨林：雾中林线、斜阳与流萤 ---------- */
const FOREST = `${SVG_HEAD}
<defs>
<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#f2eecf'/><stop offset='.4' stop-color='#d3e7c6'/><stop offset='1' stop-color='#8fbe9b'/>
</linearGradient>
<radialGradient id='sun' cx='.5' cy='.5' r='.5'>
<stop offset='0' stop-color='#fffbe6' stop-opacity='.9'/><stop offset='1' stop-color='#fffbe6' stop-opacity='0'/>
</radialGradient>
<filter id='fog' x='-30%' y='-60%' width='160%' height='220%'><feGaussianBlur stdDeviation='16'/></filter>
</defs>
<rect width='1600' height='1000' fill='url(#sky)'/>
<circle cx='480' cy='280' r='260' fill='url(#sun)'/>
<g fill='#ffffff' opacity='.16'>
<polygon points='300,0 420,0 760,620 660,620'/><polygon points='520,0 600,0 900,560 830,560'/><polygon points='120,0 190,0 560,600 490,600'/>
</g>
<path d='M0 520 L90 462 170 510 260 440 350 505 450 448 540 508 640 452 730 512 830 456 930 514 1030 460 1130 516 1230 458 1330 512 1430 462 1530 510 1600 470 V 1000 H 0 Z' fill='#b3d0a8' opacity='.85'/>
<rect y='548' width='1600' height='56' fill='#ffffff' opacity='.38' filter='url(#fog)'/>
<path d='M0 620 L110 556 220 610 340 540 460 606 580 548 700 614 820 552 940 616 1060 552 1180 618 1300 556 1420 620 1600 560 V 1000 H 0 Z' fill='#7fae83'/>
<rect y='660' width='1600' height='60' fill='#ffffff' opacity='.3' filter='url(#fog)'/>
<path d='M0 726 L130 656 260 718 400 640 540 712 680 648 820 720 960 654 1100 722 1240 656 1380 724 1600 660 V 1000 H 0 Z' fill='#4c8163'/>
<g fill='#fff6cf'>
<circle cx='420' cy='700' r='3' opacity='.8'/><circle cx='760' cy='660' r='2.4' opacity='.7'/><circle cx='1050' cy='712' r='3.2' opacity='.75'/><circle cx='1290' cy='680' r='2.2' opacity='.65'/><circle cx='620' cy='760' r='2.6' opacity='.6'/><circle cx='920' cy='780' r='2' opacity='.55'/>
</g>
<rect y='790' width='1600' height='52' fill='#ffffff' opacity='.22' filter='url(#fog)'/>
<path d='M0 846 L150 772 300 838 470 758 640 834 810 764 980 840 1150 772 1320 842 1470 782 1600 836 V 1000 H 0 Z' fill='#2a5442'/>
<path d='M0 936 L200 884 400 928 620 878 840 932 1060 886 1280 934 1600 890 V 1000 H 0 Z' fill='#16352a'/>
</svg>`;

/* ---------- 初雪：冬日暮雪、暖灯与小屋 ---------- */
const SNOW = `${SVG_HEAD}
<defs>
<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#57689f'/><stop offset='.5' stop-color='#8b9cc9'/><stop offset='1' stop-color='#e2e6f2'/>
</linearGradient>
<filter id='soft' x='-30%' y='-30%' width='160%' height='160%'><feGaussianBlur stdDeviation='12'/></filter>
<radialGradient id='lamp' cx='.5' cy='.5' r='.5'>
<stop offset='0' stop-color='#ffd98a' stop-opacity='.55'/><stop offset='1' stop-color='#ffd98a' stop-opacity='0'/>
</radialGradient>
</defs>
<rect width='1600' height='1000' fill='url(#sky)'/>
<ellipse cx='1150' cy='230' rx='340' ry='110' fill='#aab9de' opacity='.5' filter='url(#soft)'/>
<ellipse cx='420' cy='170' rx='290' ry='95' fill='#a1b1da' opacity='.45' filter='url(#soft)'/>
<path d='M0 555 Q 300 485 560 545 Q 840 505 1120 540 Q 1380 508 1600 525 V1000 H0 Z' fill='#c5cfe9'/>
<path d='M0 665 Q 360 585 700 650 Q 1080 596 1600 635 V1000 H0 Z' fill='#abb8de'/>
<circle cx='1120' cy='632' r='66' fill='url(#lamp)'/>
<g>
<rect x='1058' y='606' width='122' height='72' fill='#3d4668'/>
<polygon points='1046,606 1192,606 1119,556' fill='#2e3654'/>
<polygon points='1046,606 1192,606 1119,560' fill='#eef2fb' opacity='.9'/>
<rect x='1084' y='628' width='26' height='28' rx='3' fill='#ffd98a'/>
<rect x='1124' y='628' width='20' height='20' rx='3' fill='#ffcf7a' opacity='.85'/>
<ellipse cx='1119' cy='682' rx='86' ry='12' fill='#e6ebf8'/>
</g>
<path d='M0 785 Q 400 702 820 780 Q 1220 730 1600 755 V1000 H0 Z' fill='#8f9ed0'/>
<g stroke='#414d78' fill='none' stroke-linecap='round'>
<path d='M245 800 C 249 748 245 706 258 664' stroke-width='8'/>
<path d='M250 726 C 284 706 304 682 310 656' stroke-width='5'/>
<path d='M248 694 C 218 674 200 654 194 632' stroke-width='5'/>
<path d='M254 672 C 282 654 296 638 304 616' stroke-width='4'/>
<path d='M1310 782 C 1313 742 1310 710 1320 678' stroke-width='7'/>
<path d='M1314 722 C 1342 706 1358 688 1364 668' stroke-width='4.5'/>
<path d='M1312 696 C 1288 680 1274 664 1270 646' stroke-width='4.5'/>
</g>
<path d='M0 885 Q 500 822 1000 876 Q 1320 838 1600 862 V1000 H0 Z' fill='#7e8cc2'/>
<g fill='#ffffff'>
<circle cx='130' cy='120' r='2.6' opacity='.85'/><circle cx='310' cy='240' r='2' opacity='.65'/><circle cx='520' cy='90' r='2.4' opacity='.8'/><circle cx='700' cy='200' r='1.8' opacity='.55'/><circle cx='880' cy='120' r='2.6' opacity='.85'/><circle cx='1060' cy='260' r='2' opacity='.6'/><circle cx='1230' cy='90' r='2.4' opacity='.8'/><circle cx='1420' cy='210' r='2' opacity='.6'/><circle cx='1540' cy='110' r='2.6' opacity='.85'/><circle cx='90' cy='360' r='2.2' opacity='.7'/><circle cx='420' cy='380' r='1.8' opacity='.5'/><circle cx='640' cy='330' r='2.4' opacity='.75'/><circle cx='960' cy='390' r='2' opacity='.6'/><circle cx='1180' cy='340' r='2.2' opacity='.7'/><circle cx='1470' cy='380' r='1.8' opacity='.55'/><circle cx='260' cy='500' r='2.2' opacity='.65'/><circle cx='560' cy='480' r='2' opacity='.6'/><circle cx='860' cy='520' r='2.4' opacity='.7'/><circle cx='1120' cy='470' r='1.8' opacity='.5'/><circle cx='1390' cy='510' r='2.2' opacity='.65'/><circle cx='200' cy='620' r='2' opacity='.55'/><circle cx='1520' cy='600' r='2' opacity='.5'/>
</g>
</svg>`;

/* ---------- 大漠：长河落日、沙丘驼影 ---------- */
const DESERT = `${SVG_HEAD}
<defs>
<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#ffe9bd'/><stop offset='.45' stop-color='#fbc06a'/><stop offset='.75' stop-color='#f0934e'/><stop offset='1' stop-color='#e87e4a'/>
</linearGradient>
<radialGradient id='sun' cx='.5' cy='.5' r='.5'>
<stop offset='0' stop-color='#fff6d2' stop-opacity='.95'/><stop offset='.55' stop-color='#ffe1a0' stop-opacity='.45'/><stop offset='1' stop-color='#ffe1a0' stop-opacity='0'/>
</radialGradient>
</defs>
<rect width='1600' height='1000' fill='url(#sky)'/>
<circle cx='780' cy='560' r='330' fill='url(#sun)'/>
<circle cx='780' cy='560' r='185' fill='#fff3cd'/>
<g stroke='#7a3a20' stroke-width='5' fill='none' stroke-linecap='round' opacity='.75'>
<path d='M300 190 q 16 -16 32 0'/><path d='M352 164 q 14 -14 28 0'/><path d='M1180 150 q 15 -15 30 0'/>
</g>
<path d='M0 622 Q 400 562 800 612 Q 1200 562 1600 585 V1000 H0 Z' fill='#f6b65f'/>
<path d='M0 722 Q 450 642 900 702 Q 1280 650 1600 682 V1000 H0 Z' fill='#e28e45'/>
<path d='M0 722 Q 450 642 900 702' stroke='#ffd9a0' stroke-width='4' fill='none' opacity='.6'/>
<g fill='#5e2a1a'>
<ellipse cx='560' cy='678' rx='17' ry='8'/><circle cx='575' cy='666' r='4.5'/><circle cx='552' cy='671' r='5.5'/><circle cx='566' cy='671' r='5.5'/><rect x='549' y='682' width='2.6' height='13'/><rect x='558' y='684' width='2.6' height='12'/><rect x='566' y='684' width='2.6' height='13'/><rect x='572' y='682' width='2.6' height='12'/>
<ellipse cx='622' cy='684' rx='15' ry='7'/><circle cx='635' cy='673' r='4'/><circle cx='616' cy='678' r='5'/><circle cx='627' cy='678' r='5'/><rect x='613' y='688' width='2.4' height='12'/><rect x='628' y='688' width='2.4' height='12'/>
<ellipse cx='676' cy='680' rx='16' ry='7.5'/><circle cx='690' cy='669' r='4.2'/><circle cx='669' cy='674' r='5'/><circle cx='681' cy='674' r='5'/><rect x='665' y='684' width='2.5' height='13'/><rect x='681' y='684' width='2.5' height='12'/>
</g>
<path d='M0 835 Q 500 735 1000 822 Q 1320 775 1600 795 V1000 H0 Z' fill='#c06b38'/>
<path d='M1000 822 Q 1320 775 1600 795 V1000 H1000 Z' fill='#a04e2c' opacity='.5'/>
<path d='M0 930 Q 560 856 1120 922 Q 1400 890 1600 906 V1000 H0 Z' fill='#8f3f26'/>
</svg>`;

/* ---------- 樱雨：春山樱树、落英缤纷 ---------- */
const SAKURA = `${SVG_HEAD}
<defs>
<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#fce7f0'/><stop offset='.5' stop-color='#f8cfe2'/><stop offset='1' stop-color='#d5e2ee'/>
</linearGradient>
<filter id='soft' x='-30%' y='-30%' width='160%' height='160%'><feGaussianBlur stdDeviation='10'/></filter>
</defs>
<rect width='1600' height='1000' fill='url(#sky)'/>
<path d='M160 580 Q 470 300 790 580 Z' fill='#8494c8' opacity='.85'/>
<path d='M430 398 Q 470 366 512 396 Q 470 344 430 398' fill='#f6f8ff'/>
<path d='M368 470 Q 470 380 574 470 Q 470 424 368 470 Z' fill='#f6f8ff' opacity='.9'/>
<ellipse cx='480' cy='596' rx='430' ry='46' fill='#fce7f0' opacity='.8' filter='url(#soft)'/>
<path d='M0 700 Q 400 650 800 688 Q 1220 646 1600 680 V1000 H0 Z' fill='#f0cbde'/>
<path d='M0 820 Q 500 772 1000 812 Q 1330 782 1600 806 V1000 H0 Z' fill='#eab4cf'/>
<g>
<path d='M235 1000 C 244 892 238 806 268 726 C 276 706 288 690 304 678' stroke='#5f4348' stroke-width='17' fill='none' stroke-linecap='round'/>
<path d='M258 800 C 300 780 330 754 344 722' stroke='#5f4348' stroke-width='9' fill='none' stroke-linecap='round'/>
<path d='M266 754 C 240 730 226 706 222 680' stroke='#5f4348' stroke-width='8' fill='none' stroke-linecap='round'/>
<g fill='#f6b9d3'>
<circle cx='320' cy='650' r='86'/><circle cx='420' cy='606' r='70'/><circle cx='250' cy='600' r='64'/><circle cx='380' cy='688' r='58'/>
</g>
<g fill='#f19fbf' opacity='.85'>
<circle cx='360' cy='640' r='52'/><circle cx='282' cy='668' r='46'/><circle cx='452' cy='650' r='40'/>
</g>
<g fill='#fbd7e6' opacity='.9'>
<circle cx='340' cy='596' r='40'/><circle cx='300' cy='708' r='34'/>
</g>
</g>
<g>
<path d='M1420 1000 C 1412 912 1418 848 1394 786' stroke='#5f4348' stroke-width='14' fill='none' stroke-linecap='round'/>
<g fill='#f6b9d3'>
<circle cx='1370' cy='742' r='66'/><circle cx='1444' cy='706' r='54'/><circle cx='1320' cy='688' r='46'/>
</g>
<g fill='#f19fbf' opacity='.85'>
<circle cx='1408' cy='748' r='40'/><circle cx='1348' cy='726' r='36'/>
</g>
</g>
<g fill='#f5aacd'>
<ellipse cx='620' cy='420' rx='8' ry='4.5' opacity='.8' transform='rotate(28 620 420)'/><ellipse cx='760' cy='330' rx='7' ry='4' opacity='.6' transform='rotate(-20 760 330)'/><ellipse cx='900' cy='470' rx='8' ry='4.5' opacity='.75' transform='rotate(40 900 470)'/><ellipse cx='1040' cy='380' rx='7' ry='4' opacity='.65' transform='rotate(-30 1040 380)'/><ellipse cx='1150' cy='520' rx='8' ry='4.5' opacity='.8' transform='rotate(18 1150 520)'/><ellipse cx='540' cy='560' rx='7' ry='4' opacity='.7' transform='rotate(-42 540 560)'/><ellipse cx='980' cy='610' rx='8' ry='4.5' opacity='.7' transform='rotate(24 980 610)'/><ellipse cx='1230' cy='440' rx='7' ry='4' opacity='.6' transform='rotate(-16 1230 440)'/><ellipse cx='700' cy='700' rx='8' ry='4.5' opacity='.75' transform='rotate(32 700 700)'/><ellipse cx='1330' cy='560' rx='7' ry='4' opacity='.7' transform='rotate(-24 1330 560)'/><ellipse cx='440' cy='470' rx='7' ry='4' opacity='.6' transform='rotate(12 440 470)'/><ellipse cx='1090' cy='700' rx='8' ry='4.5' opacity='.7' transform='rotate(-36 1090 700)'/><ellipse cx='820' cy='240' rx='7' ry='4' opacity='.55' transform='rotate(22 820 240)'/><ellipse cx='1460' cy='640' rx='7' ry='4' opacity='.65' transform='rotate(-12 1460 640)'/>
</g>
</svg>`;

/* ---------- 深海：光柱入海、鲸与流萤水母 ---------- */
const ABYSS = `${SVG_HEAD}
<defs>
<linearGradient id='sea' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#0a4460'/><stop offset='.45' stop-color='#05293c'/><stop offset='1' stop-color='#02111b'/>
</linearGradient>
<filter id='soft' x='-40%' y='-40%' width='180%' height='180%'><feGaussianBlur stdDeviation='14'/></filter>
</defs>
<rect width='1600' height='1000' fill='url(#sea)'/>
<g fill='#bfeef2'>
<polygon points='240,0 400,0 640,560 460,560' opacity='.1'/>
<polygon points='700,0 800,0 1020,620 890,620' opacity='.08'/>
<polygon points='1180,0 1300,0 1560,600 1420,600' opacity='.09'/>
</g>
<g fill='#0e4d60'>
<path d='M180 420 C 240 372 380 344 500 366 C 585 382 640 376 680 362 C 666 404 610 436 520 448 C 400 462 260 454 180 420 Z'/>
<path d='M680 362 C 715 338 752 332 788 340 C 760 356 750 374 762 396 C 726 392 694 380 680 362 Z'/>
<path d='M400 440 C 420 472 456 486 494 482 C 464 498 424 496 398 480 Z'/>
</g>
<g fill='#114e5e'>
<ellipse cx='1120' cy='700' rx='7' ry='2.6'/><ellipse cx='1150' cy='688' rx='7' ry='2.6'/><ellipse cx='1182' cy='676' rx='7' ry='2.6'/><ellipse cx='1212' cy='664' rx='7' ry='2.6'/><ellipse cx='1242' cy='652' rx='7' ry='2.6'/><ellipse cx='1136' cy='662' rx='6' ry='2.2'/><ellipse cx='1168' cy='650' rx='6' ry='2.2'/><ellipse cx='1200' cy='638' rx='6' ry='2.2'/><ellipse cx='1230' cy='626' rx='6' ry='2.2'/><ellipse cx='1104' cy='676' rx='6' ry='2.2'/><ellipse cx='1262' cy='614' rx='6' ry='2.2'/><ellipse cx='1190' cy='612' rx='6' ry='2.2'/>
</g>
<g fill='#9fe8e2'>
<circle cx='340' cy='700' r='26' opacity='.16'/>
<path d='M320 700 a 20 15 0 0 1 40 0 q -10 8 -20 8 q -10 0 -20 -8 Z' opacity='.55'/>
<g stroke='#9fe8e2' stroke-width='2.5' fill='none' opacity='.45'>
<path d='M328 708 q -4 24 2 44'/><path d='M340 708 q 2 26 -2 48'/><path d='M352 708 q 6 22 0 42'/>
</g>
<circle cx='520' cy='820' r='20' opacity='.13'/>
<path d='M504 820 a 15 11 0 0 1 30 0 q -8 6 -15 6 q -7 0 -15 -6 Z' opacity='.5'/>
<g stroke='#9fe8e2' stroke-width='2' fill='none' opacity='.4'>
<path d='M510 826 q -3 18 2 34'/><path d='M522 826 q 2 20 -2 36'/>
</g>
</g>
<g fill='#cfeef2' opacity='.8'>
<circle cx='920' cy='300' r='4' opacity='.3'/><circle cx='945' cy='260' r='3' opacity='.25'/><circle cx='905' cy='230' r='2.5' opacity='.2'/><circle cx='960' cy='200' r='3.4' opacity='.3'/><circle cx='928' cy='164' r='2.4' opacity='.22'/><circle cx='1300' cy='380' r='3.4' opacity='.28'/><circle cx='1322' cy='330' r='2.6' opacity='.22'/><circle cx='1285' cy='295' r='3' opacity='.25'/><circle cx='240' cy='560' r='3' opacity='.25'/><circle cx='262' cy='512' r='2.4' opacity='.2'/><circle cx='700' cy='740' r='3' opacity='.22'/><circle cx='724' cy='700' r='2.4' opacity='.18'/><circle cx='680' cy='660' r='2.8' opacity='.2'/><circle cx='1480' cy='760' r='3.2' opacity='.24'/><circle cx='1452' cy='712' r='2.4' opacity='.2'/>
</g>
<g stroke='#0d443e' stroke-width='9' fill='none' stroke-linecap='round'>
<path d='M180 1000 C 172 920 196 860 184 790 C 178 750 190 720 186 690'/>
<path d='M260 1000 C 268 930 250 880 264 820 C 272 780 262 750 268 720'/>
<path d='M120 1000 C 116 940 128 900 122 850'/>
</g>
<path d='M0 950 Q 400 912 800 944 Q 1220 908 1600 940 V1000 H0 Z' fill='#010b12'/>
<ellipse cx='380' cy='972' rx='120' ry='18' fill='#0a3a40' opacity='.6'/>
<ellipse cx='1240' cy='978' rx='150' ry='20' fill='#0a3a40' opacity='.6'/>
</svg>`;

/* ---------- 麦浪：金色麦田、暮色云雀 ---------- */
const WHEAT = `${SVG_HEAD}
<defs>
<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#8fb8dc'/><stop offset='.5' stop-color='#c9dccf'/><stop offset='1' stop-color='#f6e6b4'/>
</linearGradient>
<radialGradient id='sun' cx='.5' cy='.5' r='.5'>
<stop offset='0' stop-color='#fff6cf' stop-opacity='.85'/><stop offset='1' stop-color='#fff6cf' stop-opacity='0'/>
</radialGradient>
<filter id='soft' x='-30%' y='-30%' width='160%' height='160%'><feGaussianBlur stdDeviation='9'/></filter>
</defs>
<rect width='1600' height='1000' fill='url(#sky)'/>
<circle cx='520' cy='540' r='230' fill='url(#sun)'/>
<ellipse cx='1080' cy='180' rx='260' ry='26' fill='#ffffff' opacity='.75' filter='url(#soft)'/>
<ellipse cx='1260' cy='240' rx='190' ry='18' fill='#ffffff' opacity='.6' filter='url(#soft)'/>
<ellipse cx='380' cy='250' rx='220' ry='20' fill='#ffffff' opacity='.55' filter='url(#soft)'/>
<g stroke='#54644a' stroke-width='4.5' fill='none' stroke-linecap='round' opacity='.7'>
<path d='M820 200 q 15 -15 30 0'/><path d='M866 176 q 13 -13 26 0'/><path d='M1180 320 q 14 -14 28 0'/>
</g>
<path d='M0 622 Q 400 596 800 616 Q 1220 592 1600 610 V1000 H0 Z' fill='#e3bd60'/>
<g>
<path d='M236 622 C 240 596 236 574 246 552 C 252 540 262 532 274 526' stroke='#5d4c26' stroke-width='10' fill='none' stroke-linecap='round'/>
<g fill='#6e5a2f'>
<circle cx='296' cy='512' r='42'/><circle cx='252' cy='498' r='34'/><circle cx='336' cy='540' r='30'/><circle cx='282' cy='548' r='38'/>
</g>
<circle cx='290' cy='524' r='26' fill='#7d683a' opacity='.8'/>
</g>
<path d='M0 720 Q 420 676 840 712 Q 1240 676 1600 704 V1000 H0 Z' fill='#d5a94e'/>
<path d='M0 720 Q 420 676 840 712' stroke='#f0d189' stroke-width='4' fill='none' opacity='.7'/>
<path d='M0 820 Q 500 762 1000 812 Q 1330 776 1600 800 V1000 H0 Z' fill='#c1913a'/>
<g stroke='#8a6528' stroke-width='2.6' stroke-linecap='round'>
<path d='M120 960 C 122 930 118 906 124 884'/><path d='M210 972 C 212 942 208 918 214 896'/><path d='M300 952 C 302 922 298 898 304 876'/><path d='M390 976 C 392 946 388 922 394 900'/><path d='M480 958 C 482 928 478 904 484 882'/><path d='M575 978 C 577 948 573 924 579 902'/><path d='M670 960 C 672 930 668 906 674 884'/><path d='M760 980 C 762 950 758 926 764 904'/><path d='M855 962 C 857 932 853 908 859 886'/><path d='M950 982 C 952 952 948 928 954 906'/><path d='M1045 964 C 1047 934 1043 910 1049 888'/><path d='M1140 984 C 1142 954 1138 930 1144 908'/><path d='M1235 966 C 1237 936 1233 912 1239 890'/><path d='M1330 986 C 1332 956 1328 932 1334 910'/><path d='M1420 968 C 1422 938 1418 914 1424 892'/><path d='M1510 988 C 1512 958 1508 934 1514 912'/>
</g>
<g fill='#d8b46a'>
<ellipse cx='124' cy='882' rx='5' ry='9' transform='rotate(8 124 882)'/><ellipse cx='214' cy='894' rx='5' ry='9' transform='rotate(-6 214 894)'/><ellipse cx='304' cy='874' rx='5' ry='9' transform='rotate(10 304 874)'/><ellipse cx='394' cy='898' rx='5' ry='9' transform='rotate(-8 394 898)'/><ellipse cx='484' cy='880' rx='5' ry='9' transform='rotate(6 484 880)'/><ellipse cx='579' cy='900' rx='5' ry='9' transform='rotate(-10 579 900)'/><ellipse cx='674' cy='882' rx='5' ry='9' transform='rotate(7 674 882)'/><ellipse cx='764' cy='902' rx='5' ry='9' transform='rotate(-7 764 902)'/><ellipse cx='859' cy='884' rx='5' ry='9' transform='rotate(9 859 884)'/><ellipse cx='954' cy='904' rx='5' ry='9' transform='rotate(-9 954 904)'/><ellipse cx='1049' cy='886' rx='5' ry='9' transform='rotate(5 1049 886)'/><ellipse cx='1144' cy='906' rx='5' ry='9' transform='rotate(-8 1144 906)'/><ellipse cx='1239' cy='888' rx='5' ry='9' transform='rotate(8 1239 888)'/><ellipse cx='1334' cy='908' rx='5' ry='9' transform='rotate(-6 1334 908)'/><ellipse cx='1424' cy='890' rx='5' ry='9' transform='rotate(9 1424 890)'/><ellipse cx='1514' cy='910' rx='5' ry='9' transform='rotate(-7 1514 910)'/>
</g>
</svg>`;

/* ---------- 萤火：夏夜塘畔、流萤与月 ---------- */
const FIREFLY = `${SVG_HEAD}
<defs>
<linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'>
<stop offset='0' stop-color='#152f3e'/><stop offset='.42' stop-color='#2e4f55'/><stop offset='.72' stop-color='#b5714a'/><stop offset='1' stop-color='#e2a96b'/>
</linearGradient>
<radialGradient id='glow' cx='.5' cy='.5' r='.5'>
<stop offset='0' stop-color='#ffe89a' stop-opacity='.9'/><stop offset='.4' stop-color='#ffe89a' stop-opacity='.25'/><stop offset='1' stop-color='#ffe89a' stop-opacity='0'/>
</radialGradient>
</defs>
<rect width='1600' height='1000' fill='url(#sky)'/>
<circle cx='1270' cy='170' r='40' fill='url(#glow)'/>
<circle cx='1270' cy='170' r='26' fill='#ffedbe' opacity='.95'/>
<g fill='#fff3d2'>
<circle cx='150' cy='110' r='1.4' opacity='.6'/><circle cx='330' cy='70' r='1.1' opacity='.45'/><circle cx='520' cy='150' r='1.2' opacity='.5'/><circle cx='760' cy='90' r='1.3' opacity='.55'/><circle cx='960' cy='150' r='1' opacity='.4'/><circle cx='1450' cy='90' r='1.2' opacity='.5'/><circle cx='1560' cy='190' r='1' opacity='.4'/><circle cx='420' cy='220' r='1' opacity='.4'/>
</g>
<path d='M0 640 L90 608 180 634 280 596 380 632 480 600 580 636 680 602 780 638 880 604 980 638 1080 606 1180 640 1280 606 1380 636 1480 608 1600 634 V1000 H0 Z' fill='#122b2e'/>
<path d='M0 730 Q 400 700 800 724 Q 1220 696 1600 720 V1000 H0 Z' fill='#0e2428'/>
<rect y='790' width='1600' height='210' fill='#0a1d22'/>
<g stroke='#3f6a68' stroke-width='2.5' opacity='.45'>
<line x1='240' y1='828' x2='560' y2='828'/><line x1='320' y1='862' x2='700' y2='862'/><line x1='180' y1='900' x2='480' y2='900'/><line x1='900' y1='840' x2='1240' y2='840'/><line x1='980' y1='880' x2='1420' y2='880'/><line x1='860' y1='922' x2='1160' y2='922'/>
</g>
<g stroke='#ffedbe' opacity='.3' stroke-width='3' stroke-linecap='round'>
<line x1='1266' y1='812' x2='1266' y2='836'/><line x1='1266' y1='856' x2='1266' y2='884'/><line x1='1270' y1='906' x2='1270' y2='940'/>
</g>
<g stroke='#081518' stroke-width='4' fill='none' stroke-linecap='round'>
<path d='M100 1000 C 106 930 100 870 112 810 C 118 778 114 750 118 724'/><path d='M150 1000 C 152 940 148 896 158 848'/><path d='M70 1000 C 72 950 68 912 76 868'/><path d='M1460 1000 C 1454 926 1462 862 1450 800 C 1444 768 1450 740 1446 714'/><path d='M1512 1000 C 1516 944 1510 900 1520 852'/><path d='M1408 1000 C 1406 952 1410 914 1402 872'/>
</g>
<g fill='#ffe89a'>
<circle cx='300' cy='690' r='11' opacity='.16'/><circle cx='300' cy='690' r='2.8'/>
<circle cx='480' cy='740' r='13' opacity='.15'/><circle cx='480' cy='740' r='3'/>
<circle cx='640' cy='650' r='10' opacity='.16'/><circle cx='640' cy='650' r='2.4'/>
<circle cx='820' cy='712' r='12' opacity='.15'/><circle cx='820' cy='712' r='2.8'/>
<circle cx='980' cy='664' r='10' opacity='.17'/><circle cx='980' cy='664' r='2.4'/>
<circle cx='1140' cy='736' r='13' opacity='.14'/><circle cx='1140' cy='736' r='3'/>
<circle cx='420' cy='616' r='9' opacity='.15'/><circle cx='420' cy='616' r='2.2'/>
<circle cx='760' cy='580' r='9' opacity='.14'/><circle cx='760' cy='580' r='2.2'/>
<circle cx='1060' cy='600' r='10' opacity='.15'/><circle cx='1060' cy='600' r='2.4'/>
<circle cx='240' cy='800' r='11' opacity='.14'/><circle cx='240' cy='800' r='2.6'/>
<circle cx='580' cy='828' r='10' opacity='.13'/><circle cx='580' cy='828' r='2.4'/>
<circle cx='900' cy='808' r='11' opacity='.14'/><circle cx='900' cy='808' r='2.6'/>
<circle cx='1220' cy='790' r='9' opacity='.13'/><circle cx='1220' cy='790' r='2.2'/>
<circle cx='1380' cy='700' r='10' opacity='.15'/><circle cx='1380' cy='700' r='2.4'/>
<circle cx='1320' cy='880' r='10' opacity='.12'/><circle cx='1320' cy='880' r='2.4'/>
</g>
</svg>`;

/** 深色主题纱的快捷构造：自上而下三段压暗 */
const darkScrim = (a1: number, a2: number, a3: number) =>
  `linear-gradient(180deg, rgba(12,10,18,${a1}) 0%, rgba(12,10,18,${a2}) 55%, rgba(12,10,18,${a3}) 100%)`;
/** 浅色主题纱的快捷构造：自上而下两段提亮 */
const lightScrim = (a1: number, a2: number) =>
  `linear-gradient(180deg, rgba(252,252,254,${a1}) 0%, rgba(252,252,254,${a2}) 100%)`;

export const SKINS: Skin[] = [
  // 每套皮肤按自身明暗分布单独调纱：深色场景轻压暗+重提亮，明亮场景反之
  {
    key: "starry", name: "星夜", desc: "静谧深蓝 · 山月相伴", svg: STARRY,
    scrimDark: darkScrim(0.1, 0.24, 0.38), scrimLight: lightScrim(0.74, 0.82),
  },
  {
    key: "aurora", name: "极光", desc: "寒夜雪原 · 光幕流动", svg: AURORA,
    scrimDark: darkScrim(0.14, 0.3, 0.44), scrimLight: lightScrim(0.72, 0.8),
  },
  {
    key: "sunset", name: "晚霞", desc: "海面落日 · 云霞归鸟", svg: SUNSET,
    scrimDark: darkScrim(0.42, 0.56, 0.66), scrimLight: lightScrim(0.36, 0.5),
  },
  {
    key: "ink", name: "水墨", desc: "远山淡影 · 留白朱砂", svg: INK,
    scrimDark: darkScrim(0.48, 0.62, 0.7), scrimLight: lightScrim(0.22, 0.38),
  },
  {
    key: "neon", name: "霓虹", desc: "合成波 · 网格都市", svg: NEON,
    scrimDark: darkScrim(0.16, 0.32, 0.44), scrimLight: lightScrim(0.74, 0.82),
  },
  {
    key: "forest", name: "晨林", desc: "雾中林线 · 斜阳流萤", svg: FOREST,
    scrimDark: darkScrim(0.28, 0.44, 0.56), scrimLight: lightScrim(0.46, 0.6),
  },
  {
    key: "snow", name: "初雪", desc: "冬日暮雪 · 灯火可亲", svg: SNOW,
    scrimDark: darkScrim(0.5, 0.64, 0.72), scrimLight: lightScrim(0.2, 0.36),
  },
  {
    key: "desert", name: "大漠", desc: "长河落日 · 沙丘驼影", svg: DESERT,
    scrimDark: darkScrim(0.42, 0.58, 0.68), scrimLight: lightScrim(0.32, 0.46),
  },
  {
    key: "sakura", name: "樱雨", desc: "春山樱雪 · 落英缤纷", svg: SAKURA,
    scrimDark: darkScrim(0.44, 0.58, 0.66), scrimLight: lightScrim(0.24, 0.4),
  },
  {
    key: "abyss", name: "深海", desc: "光柱入海 · 鲸游深蓝", svg: ABYSS,
    scrimDark: darkScrim(0.1, 0.26, 0.38), scrimLight: lightScrim(0.74, 0.82),
  },
  {
    key: "wheat", name: "麦浪", desc: "金色麦田 · 暮色四合", svg: WHEAT,
    scrimDark: darkScrim(0.4, 0.56, 0.64), scrimLight: lightScrim(0.34, 0.5),
  },
  {
    key: "firefly", name: "萤火", desc: "夏夜塘畔 · 流萤点点", svg: FIREFLY,
    scrimDark: darkScrim(0.28, 0.46, 0.56), scrimLight: lightScrim(0.54, 0.66),
  },
];

const SKIN_KEY = "rustmusic.skin";
export const DEFAULT_SKIN = "default";

const URI_CACHE = new Map<string, string>();

/** 皮肤背景的 data URI；默认皮肤返回 null（走内置渐变氛围） */
export function skinUri(key: string): string | null {
  if (!key || key === DEFAULT_SKIN) return null;
  const cached = URI_CACHE.get(key);
  if (cached) return cached;
  const skin = SKINS.find((s) => s.key === key);
  if (!skin) return null;
  const uri = `url("data:image/svg+xml,${encodeURIComponent(skin.svg)}")`;
  URI_CACHE.set(key, uri);
  return uri;
}

export function applySkin(key: string) {
  const root = document.documentElement.style;
  const uri = skinUri(key);
  if (uri) {
    root.setProperty("--skin-url", uri);
    // 每套皮肤各自的双主题纱：CSS 按 html[data-theme] 取用对应的一份
    const skin = SKINS.find((s) => s.key === key);
    if (skin) {
      root.setProperty("--skin-scrim-dark", skin.scrimDark);
      root.setProperty("--skin-scrim-light", skin.scrimLight);
    }
  } else {
    root.removeProperty("--skin-url");
    root.removeProperty("--skin-scrim-dark");
    root.removeProperty("--skin-scrim-light");
  }
  // data-skin 标记：CSS 据此切换玻璃/播放条的透明度档位——皮肤启用时
  // 内容大卡片与播放条更透、磨砂更轻，壁纸左右透出一致（默认皮肤不变）
  document.documentElement.dataset.skin = uri ? "on" : "off";
}

export function loadSkin(): string {
  return localStorage.getItem(SKIN_KEY) ?? DEFAULT_SKIN;
}

export function saveSkin(key: string) {
  localStorage.setItem(SKIN_KEY, key);
}
