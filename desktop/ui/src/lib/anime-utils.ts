/**
 * Утилиты для Anime.js — стиль анимаций как на https://animejs.com/documentation/animation/
 */
import { animate, stagger } from 'animejs';

export { stagger };

/** Анимация появления снизу вверх (fade-in-up) для одного элемента */
export function animateFadeInUp(
  target: Element | string | NodeListOf<Element>,
  options?: { delay?: number; duration?: number }
) {
  return animate(target, {
    opacity: [0, 1],
    translateY: [24, 0],
    duration: options?.duration ?? 600,
    delay: options?.delay ?? 0,
    ease: 'outExpo',
  });
}

/** Stagger-анимация для списка элементов (появление снизу) */
export function animateStaggerIn(
  target: string | NodeListOf<Element>,
  options?: { staggerDelay?: number; duration?: number }
) {
  return animate(target, {
    opacity: [0, 1],
    translateY: [20, 0],
    duration: options?.duration ?? 500,
    delay: stagger(options?.staggerDelay ?? 60),
    ease: 'outExpo',
  });
}

/** Мягкое появление (только opacity) */
export function animateFadeIn(
  target: Element | string | NodeListOf<Element>,
  options?: { delay?: number; duration?: number }
) {
  return animate(target, {
    opacity: [0, 1],
    duration: options?.duration ?? 400,
    delay: options?.delay ?? 0,
    ease: 'outQuad',
  });
}

/** Анимация логотипа/иконки при загрузке */
export function animateLogo(target: Element | string) {
  return animate(target, {
    opacity: [0, 1],
    scale: [0.92, 1],
    duration: 700,
    ease: 'outExpo',
  });
}

/** Карточки панели управления — stagger с лёгким подъёмом */
export function animateCardsStagger(target: string | NodeListOf<Element>) {
  return animate(target, {
    opacity: [0, 1],
    translateY: [32, 0],
    duration: 600,
    delay: stagger(120, { start: 0 }),
    ease: 'outExpo',
  });
}
