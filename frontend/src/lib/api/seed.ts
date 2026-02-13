import { createOrder, submitOrder, fulfillOrder } from './orders';

export type SeedProgressCallback = (step: number, total: number, label: string) => void;

export async function seedDemoData(onProgress?: SeedProgressCallback): Promise<void> {
	const total = 3;

	// 1. Draft order — create with items, leave as Draft
	onProgress?.(1, total, 'Creating draft order...');
	await createOrder({
		items: [
			{ product_id: 'SKU-WIDGET', product_name: 'Premium Widget', quantity: 3, unit_price_cents: 2499 },
			{ product_id: 'SKU-GADGET', product_name: 'Super Gadget', quantity: 1, unit_price_cents: 4999 }
		]
	});

	// 2. Reserved order — create with items, then submit
	onProgress?.(2, total, 'Creating submitted order...');
	const submitted = await createOrder({
		items: [
			{ product_id: 'SKU-BOLT', product_name: 'Titanium Bolt', quantity: 10, unit_price_cents: 350 },
			{ product_id: 'SKU-NUT', product_name: 'Titanium Nut', quantity: 10, unit_price_cents: 250 }
		]
	});
	await submitOrder(submitted.order_id);

	// 3. Completed order — create, submit, fulfill via saga
	onProgress?.(3, total, 'Creating fulfilled order...');
	const fulfilled = await createOrder({
		items: [
			{ product_id: 'SKU-PLATE', product_name: 'Steel Plate', quantity: 2, unit_price_cents: 7500 },
			{ product_id: 'SKU-RIVET', product_name: 'Copper Rivet', quantity: 50, unit_price_cents: 120 }
		]
	});
	await submitOrder(fulfilled.order_id);
	await fulfillOrder(fulfilled.order_id);
}
